//! The fallback on TCP (decision 0017): the same calls over Connect, gRPC and gRPC-Web, from the
//! same `.proto`, for networks that block UDP and for clients that speak only gRPC (a Python
//! notebook, another language's generated stub). One unary method,
//! `POST /kitchen.runtime.v1.Runtime/Call`, answered by hand on hyper: each protocol is a few
//! bytes of framing around the same `Request` and `Answer`.
//!
//! | Content type | Protocol | Body |
//! |---|---|---|
//! | `application/proto`, `application/json` | Connect (unary) | the message (JSON: protobuf's own mapping) |
//! | `application/grpc[+proto]` | gRPC (HTTP/2) | a 5-byte prefix, the message; `grpc-status` in trailers |
//! | `application/grpc-web[+proto]` | gRPC-Web | as gRPC, the trailers as a last frame of the body |
//!
//! The caller says who it is with `authorization: Bearer <token>` (a token from pairing over
//! WebTransport, or enrolled on the server). What the runtime answers (an op not found, a run
//! refused) is in the `Answer`, as over WebTransport; only the transport's own failures (no
//! token, not a message) are the protocol's errors.

use std::convert::Infallible;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::Frame;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use hyper::{Method, Request, Response, StatusCode};
use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor};

use kitchen_network::auth::Peer;
use kitchen_network::pair::{PairHello, PairReply, PairToken};

use crate::server::{answer, same_origin, State};
use crate::{HELLO_TIMEOUT, MAX_HELLO, MAX_REQUEST};

/// The one method's path.
pub const CALL_PATH: &str = "/kitchen.runtime.v1.Runtime/Call";

/// Pairing over HTTP, first step: a [`PairHello`] (JSON) → `{"pairing": id, "reply": PairReply}`.
pub const PAIR_START_PATH: &str = "/kitchen.runtime.v1.Pairing/Start";

/// Pairing over HTTP, last step: `{"pairing": id, "proof": PairToken}` → a [`PairReply`] (the
/// token, or why not).
pub const PAIR_FINISH_PATH: &str = "/kitchen.runtime.v1.Pairing/Finish";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Protocol {
    ConnectProto,
    ConnectJson,
    Grpc,
    GrpcWeb,
}

impl Protocol {
    fn of(content_type: &str) -> Option<Self> {
        let base = content_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
        match base.as_str() {
            "application/proto" => Some(Self::ConnectProto),
            "application/json" => Some(Self::ConnectJson),
            "application/grpc" | "application/grpc+proto" => Some(Self::Grpc),
            "application/grpc-web" | "application/grpc-web+proto" => Some(Self::GrpcWeb),
            _ => None,
        }
    }

    fn content_type(self) -> &'static str {
        match self {
            Self::ConnectProto => "application/proto",
            Self::ConnectJson => "application/json",
            Self::Grpc => "application/grpc+proto",
            Self::GrpcWeb => "application/grpc-web+proto",
        }
    }

    fn framed(self) -> bool {
        matches!(self, Self::Grpc | Self::GrpcWeb)
    }
}

/// A transport failure, said in each protocol's own terms (gRPC codes; Connect's names and HTTP
/// statuses).
#[derive(Clone, Copy, Debug)]
enum Code {
    InvalidArgument,
    Unauthenticated,
    PermissionDenied,
    ResourceExhausted,
    Unimplemented,
}

impl Code {
    fn grpc(self) -> u8 {
        match self {
            Self::InvalidArgument => 3,
            Self::PermissionDenied => 7,
            Self::ResourceExhausted => 8,
            Self::Unimplemented => 12,
            Self::Unauthenticated => 16,
        }
    }

    fn connect(self) -> (&'static str, StatusCode) {
        match self {
            Self::InvalidArgument => ("invalid_argument", StatusCode::BAD_REQUEST),
            Self::PermissionDenied => ("permission_denied", StatusCode::FORBIDDEN),
            Self::ResourceExhausted => ("resource_exhausted", StatusCode::TOO_MANY_REQUESTS),
            Self::Unimplemented => ("unimplemented", StatusCode::NOT_IMPLEMENTED),
            Self::Unauthenticated => ("unauthenticated", StatusCode::UNAUTHORIZED),
        }
    }
}

/// A body of at most one data frame and then, for gRPC, the trailers.
pub(crate) struct Reply {
    data: Option<Bytes>,
    trailers: Option<HeaderMap>,
}

impl http_body::Body for Reply {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        let this = self.get_mut();
        if let Some(data) = this.data.take() {
            return Poll::Ready(Some(Ok(Frame::data(data))));
        }
        Poll::Ready(this.trailers.take().map(|t| Ok(Frame::trailers(t))))
    }

    fn is_end_stream(&self) -> bool {
        self.data.is_none() && self.trailers.is_none()
    }
}

fn plain(status: StatusCode, text: &str) -> Response<Reply> {
    let mut response = Response::new(Reply { data: Some(Bytes::from(text.to_string())), trailers: None });
    *response.status_mut() = status;
    response.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"));
    response
}

/// The messages' descriptors, for Connect's JSON.
fn descriptors() -> &'static (MessageDescriptor, MessageDescriptor) {
    static POOL: OnceLock<(MessageDescriptor, MessageDescriptor)> = OnceLock::new();
    POOL.get_or_init(|| {
        let pool = DescriptorPool::decode(kitchen_wire::DESCRIPTOR_SET).expect("the descriptor set built with the crate");
        let get = |name: &str| pool.get_message_by_name(name).expect("a message of runtime.proto");
        (get("kitchen.runtime.v1.Request"), get("kitchen.runtime.v1.Answer"))
    })
}

/// A 5-byte gRPC prefix (not compressed) and the message.
fn frame(flag: u8, message: &[u8]) -> Bytes {
    let mut out = Vec::with_capacity(5 + message.len());
    out.push(flag);
    out.extend_from_slice(&(message.len() as u32).to_be_bytes());
    out.extend_from_slice(message);
    Bytes::from(out)
}

/// The one message of a gRPC body.
fn unframe(body: &[u8]) -> Result<&[u8], (Code, String)> {
    if body.len() < 5 {
        return Err((Code::InvalidArgument, "no message".into()));
    }
    if body[0] & 1 != 0 {
        return Err((Code::Unimplemented, "compressed messages are not accepted: send them uncompressed".into()));
    }
    let len = u32::from_be_bytes([body[1], body[2], body[3], body[4]]) as usize;
    body.get(5..5 + len).filter(|m| 5 + m.len() == body.len()).ok_or_else(|| (Code::InvalidArgument, "one message per call".into()))
}

fn grpc_trailers(code: u8, message: &str) -> HeaderMap {
    let mut trailers = HeaderMap::new();
    trailers.insert("grpc-status", HeaderValue::from(code as u16));
    if !message.is_empty() {
        // grpc-message is percent-encoded
        let encoded: String = message.bytes().map(|b| if (0x20..0x7f).contains(&b) && b != b'%' { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
        if let Ok(value) = HeaderValue::from_str(&encoded) {
            trailers.insert("grpc-message", value);
        }
    }
    trailers
}

/// A message, or a failure, in `protocol`'s terms.
fn respond(protocol: Protocol, outcome: Result<Bytes, (Code, String)>) -> Response<Reply> {
    let mut response = match (protocol, outcome) {
        (Protocol::Grpc, Ok(message)) => Response::new(Reply { data: Some(frame(0, &message)), trailers: Some(grpc_trailers(0, "")) }),
        (Protocol::Grpc, Err((code, why))) => {
            // Trailers only: the status in the headers, no body
            let mut response = Response::new(Reply { data: None, trailers: None });
            response.headers_mut().extend(grpc_trailers(code.grpc(), &why));
            response
        }
        (Protocol::GrpcWeb, outcome) => {
            let (data, trailers) = match outcome {
                Ok(message) => (Some(message), grpc_trailers(0, "")),
                Err((code, why)) => (None, grpc_trailers(code.grpc(), &why)),
            };
            let mut body = data.map(|m| frame(0, &m).to_vec()).unwrap_or_default();
            let text: String = trailers.iter().map(|(k, v)| format!("{k}: {}\r\n", v.to_str().unwrap_or(""))).collect();
            body.extend_from_slice(&frame(0x80, text.as_bytes()));
            Response::new(Reply { data: Some(Bytes::from(body)), trailers: None })
        }
        (_, Ok(message)) => Response::new(Reply { data: Some(message), trailers: None }),
        (_, Err((code, why))) => {
            let (name, status) = code.connect();
            let body = serde_json::json!({ "code": name, "message": why }).to_string();
            let mut response = Response::new(Reply { data: Some(Bytes::from(body)), trailers: None });
            *response.status_mut() = status;
            response.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            return response;
        }
    };
    response.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_static(protocol.content_type()));
    response
}

/// Answers one HTTP request; `certified`: who the connection's client certificate names.
pub(crate) async fn handle(request: Request<Incoming>, state: Arc<State>, certified: Option<Arc<Peer>>) -> Result<Response<Reply>, Infallible> {
    if matches!(request.uri().path(), PAIR_START_PATH | PAIR_FINISH_PATH) {
        return Ok(pair(request, &state).await);
    }
    if request.uri().path() != CALL_PATH {
        return Ok(plain(StatusCode::NOT_FOUND, "the runtime API is POST /kitchen.runtime.v1.Runtime/Call"));
    }
    if request.method() != Method::POST {
        return Ok(plain(StatusCode::METHOD_NOT_ALLOWED, "POST only"));
    }
    let content_type = request.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");
    let Some(protocol) = Protocol::of(content_type) else {
        return Ok(plain(StatusCode::UNSUPPORTED_MEDIA_TYPE, "application/proto or application/json (Connect), application/grpc, application/grpc-web"));
    };
    Ok(respond(protocol, call(request, &state, protocol, certified).await))
}

fn json_reply(status: StatusCode, value: &impl serde::Serialize) -> Response<Reply> {
    let mut response = Response::new(Reply { data: Some(Bytes::from(serde_json::to_vec(value).unwrap_or_default())), trailers: None });
    *response.status_mut() = status;
    response.headers_mut().insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

#[derive(serde::Deserialize)]
struct Finish {
    pairing: String,
    proof: PairToken,
}

/// Pairing with a code over HTTP, for a page or a program where UDP is blocked: the same exchange
/// as on a session's first stream, in two requests. Between them the server keeps the pairing for
/// [`HELLO_TIMEOUT`].
async fn pair(request: Request<Incoming>, state: &Arc<State>) -> Response<Reply> {
    if request.method() != Method::POST {
        return plain(StatusCode::METHOD_NOT_ALLOWED, "POST only");
    }
    let headers = request.headers();
    let host = headers.get("host").and_then(|v| v.to_str().ok()).map(str::to_string).or_else(|| request.uri().authority().map(|a| a.to_string())).unwrap_or_default();
    if !same_origin(headers.get("origin").and_then(|v| v.to_str().ok()), &host) {
        return plain(StatusCode::FORBIDDEN, "not the page's own origin");
    }
    let finish = request.uri().path() == PAIR_FINISH_PATH;
    let Ok(body) = http_body_util::Limited::new(request.into_body(), MAX_HELLO).collect().await.map(|b| b.to_bytes()) else {
        return plain(StatusCode::PAYLOAD_TOO_LARGE, "too large");
    };
    let mut pairings = state.pairings.lock().unwrap_or_else(|e| e.into_inner());
    pairings.retain(|_, (started, _)| started.elapsed() < HELLO_TIMEOUT);
    if !finish {
        let Ok(hello) = serde_json::from_slice::<PairHello>(&body) else { return plain(StatusCode::BAD_REQUEST, "not a PairHello") };
        return match state.authority.pairing_start(&hello) {
            Ok((pairing, reply)) => {
                let mut id = [0u8; 16];
                if getrandom::fill(&mut id).is_err() {
                    return plain(StatusCode::INTERNAL_SERVER_ERROR, "no randomness");
                }
                let id: String = id.iter().map(|b| format!("{b:02x}")).collect();
                pairings.insert(id.clone(), (std::time::Instant::now(), pairing));
                json_reply(StatusCode::OK, &serde_json::json!({ "pairing": id, "reply": reply }))
            }
            Err(refusal) => json_reply(StatusCode::FORBIDDEN, &refusal),
        };
    }
    let Ok(Finish { pairing, proof }) = serde_json::from_slice::<Finish>(&body) else { return plain(StatusCode::BAD_REQUEST, "not {pairing, proof}") };
    let Some((_, started)) = pairings.remove(&pairing) else {
        return json_reply(StatusCode::NOT_FOUND, &PairReply::Error { reason: "no such pairing, or it took too long: start again".into() });
    };
    drop(pairings);
    let reply = state.authority.pairing_finish_token(started, &proof, &state.names);
    let status = if matches!(reply, PairReply::Token { .. }) { StatusCode::OK } else { StatusCode::FORBIDDEN };
    json_reply(status, &reply)
}

async fn call(request: Request<Incoming>, state: &Arc<State>, protocol: Protocol, certified: Option<Arc<Peer>>) -> Result<Bytes, (Code, String)> {
    let headers = request.headers();
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let host = header("host").map(str::to_string).or_else(|| request.uri().authority().map(|a| a.to_string())).unwrap_or_default();
    if !same_origin(header("origin"), &host) {
        return Err((Code::PermissionDenied, "not the page's own origin".into()));
    }
    // A token when one is given, else the connection's client certificate
    let token = header("authorization").and_then(|v| v.strip_prefix("Bearer ")).map(str::trim).unwrap_or("");
    let peer = if token.is_empty() { certified.map(|p| (*p).clone()) } else { state.authority.authorize_token(token) };
    let Some(peer) = peer else {
        return Err((Code::Unauthenticated, "an `authorization: Bearer <token>` header with a token the server knows, or a client certificate it issued".into()));
    };
    if header("grpc-encoding").is_some_and(|e| e != "identity") || header("content-encoding").is_some_and(|e| e != "identity") {
        return Err((Code::Unimplemented, "compressed messages are not accepted".into()));
    }
    let body = http_body_util::Limited::new(request.into_body(), MAX_REQUEST + 5).collect().await.map_err(|e| (Code::ResourceExhausted, format!("the request: {e}")))?.to_bytes();
    let message: Vec<u8> = match protocol {
        _ if protocol.framed() => unframe(&body)?.to_vec(),
        Protocol::ConnectJson => {
            let mut json = serde_json::Deserializer::from_slice(&body);
            let dynamic = DynamicMessage::deserialize(descriptors().0.clone(), &mut json).map_err(|e| (Code::InvalidArgument, format!("not a Request: {e}")))?;
            dynamic.encode_to_vec()
        }
        _ => body.to_vec(),
    };
    let peer = Arc::new(peer);
    let answer = answer(state, &peer, &message).await.encode_to_vec();
    Ok(match protocol {
        Protocol::ConnectJson => {
            let dynamic = DynamicMessage::decode(descriptors().1.clone(), answer.as_slice()).map_err(|e| (Code::InvalidArgument, e.to_string()))?;
            Bytes::from(serde_json::to_vec(&dynamic).map_err(|e| (Code::InvalidArgument, e.to_string()))?)
        }
        _ => Bytes::from(answer),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocols_are_told_by_content_type() {
        assert_eq!(Protocol::of("application/grpc"), Some(Protocol::Grpc));
        assert_eq!(Protocol::of("application/grpc-web+proto"), Some(Protocol::GrpcWeb));
        assert_eq!(Protocol::of("application/json; charset=utf-8"), Some(Protocol::ConnectJson));
        assert_eq!(Protocol::of("text/html"), None);
    }

    #[test]
    fn grpc_frames_hold_one_uncompressed_message() {
        let framed = frame(0, b"abc");
        assert_eq!(unframe(&framed).unwrap(), b"abc");
        assert!(unframe(&frame(1, b"abc")).is_err());
        let mut two = framed.to_vec();
        two.extend_from_slice(&framed);
        assert!(unframe(&two).is_err());
        assert!(unframe(b"\0\0").is_err());
    }

    #[test]
    fn the_json_mapping_knows_the_messages() {
        let (request, answer) = descriptors();
        assert_eq!(request.full_name(), "kitchen.runtime.v1.Request");
        assert_eq!(answer.full_name(), "kitchen.runtime.v1.Answer");
    }
}
