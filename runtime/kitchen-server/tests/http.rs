//! The fallback on TCP: the same calls over gRPC (HTTP/2), gRPC-Web and Connect (HTTP/1.1, JSON
//! or protobuf), from a plain HTTP client, as a notebook's gRPC stub or `curl` makes them.

use std::collections::BTreeMap;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::header::HeaderMap;
use hyper::StatusCode;
use hyper_util::rt::{TokioExecutor, TokioIo};
use kitchen_client::{Answer, Caller, Job, Local, Produced, Request, Runner, Runtime, RuntimeError, Source, Target};
use kitchen_contract::{Catalog, Format, OpSpec, PortSpec, RunStatus, Type};
use kitchen_network::auth::Authority;
use kitchen_network::pair::Permission;
use kitchen_server::{Server, ServerOptions, CALL_PATH};
use prost::Message;

struct Fake;

impl Runner for Fake {
    fn run(&self, job: &Job, _: &kitchen_client::Cancel, _: &mut dyn FnMut(usize, usize)) -> Result<Vec<Produced>, String> {
        let path = job.out_dir.join("out.txt");
        std::fs::write(&path, "figure").map_err(|e| e.to_string())?;
        Ok(vec![Produced { name: "figure".into(), path, ty: Type::new("figure"), format: Format::parse("svg").unwrap() }])
    }
}

struct Served {
    addr: String,
    dir: std::path::PathBuf,
    control: String,
    read: String,
}

async fn serve(name: &str) -> Served {
    let dir = std::env::temp_dir().join(format!("kitchen-server-http-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut catalog = Catalog::new(["signal", "figure"].map(Type::new));
    catalog.add(OpSpec::new("dspk:plot", "Plot").input(PortSpec::new("signal", "signal")).output(PortSpec::new("figure", "figure"))).unwrap();
    let runtime: Arc<dyn Runtime> = Arc::new(Local::new("dspk", catalog, &dir, Arc::new(Fake), 2).unwrap());
    let authority = Arc::new(Authority::open_or_create(&dir.join("authority")).unwrap());
    let (_, control) = authority.enroll_token("ana", Permission::Control).unwrap();
    let (_, read) = authority.enroll_token("ben", Permission::Read).unwrap();
    let server = Server::bind(ServerOptions::loopback("test"), authority, runtime).unwrap();
    let base = server.http_url("127.0.0.1").unwrap().expect("TCP is on by default on loopback");
    tokio::spawn(server.run());
    Served { addr: base.trim_start_matches("http://").to_string(), dir, control, read }
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
    trailers: Option<HeaderMap>,
}

/// One POST of `body` to the call's path, over HTTP/2 (prior knowledge) or HTTP/1.1.
async fn post(addr: &str, http2: bool, content_type: &str, token: Option<&str>, body: Vec<u8>) -> Reply {
    let io = TokioIo::new(tokio::net::TcpStream::connect(addr).await.unwrap());
    let mut builder = hyper::Request::post(format!("http://{addr}{CALL_PATH}")).header("content-type", content_type).header("host", addr);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    if http2 {
        builder = builder.header("te", "trailers");
    }
    let request = builder.body(Full::new(Bytes::from(body))).unwrap();
    let response = if http2 {
        let (mut sender, connection) = hyper::client::conn::http2::handshake(TokioExecutor::new(), io).await.unwrap();
        tokio::spawn(connection);
        sender.send_request(request).await.unwrap()
    } else {
        let (mut sender, connection) = hyper::client::conn::http1::handshake(io).await.unwrap();
        tokio::spawn(connection);
        sender.send_request(request).await.unwrap()
    };
    let (parts, body) = response.into_parts();
    let collected = body.collect().await.unwrap();
    let trailers = collected.trailers().cloned();
    Reply { status: parts.status, headers: parts.headers, body: collected.to_bytes(), trailers }
}

fn grpc_frame(message: &[u8]) -> Vec<u8> {
    let mut out = vec![0];
    out.extend_from_slice(&(message.len() as u32).to_be_bytes());
    out.extend_from_slice(message);
    out
}

fn request(caller: &Caller, request: Request) -> Vec<u8> {
    kitchen_wire::encode_request(caller, &request).encode_to_vec()
}

fn answer(bytes: &[u8]) -> Result<Answer, RuntimeError> {
    kitchen_wire::parse_answer(bytes).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn grpc_grpc_web_and_connect_carry_the_same_calls() {
    let s = serve("protocols").await;
    let me = Caller::new("someone", "notebook");

    // gRPC over HTTP/2: the answer framed, grpc-status 0 in the trailers
    let reply = post(&s.addr, true, "application/grpc", Some(&s.control), grpc_frame(&request(&me, Request::Types))).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.headers["content-type"], "application/grpc+proto");
    assert_eq!(reply.trailers.as_ref().unwrap()["grpc-status"], "0");
    assert!(matches!(answer(&reply.body[5..]), Ok(Answer::Types { types }) if types.iter().any(|t| t.name == Type::new("signal"))));

    // A run over gRPC: recorded as the person the token names, through what the caller said
    let signal = s.dir.join("s.bin");
    std::fs::write(&signal, "signal").unwrap();
    let filling: BTreeMap<String, Source> = [("signal".into(), Source::Path { path: signal, ty: Type::new("signal"), format: Format::parse("raw").unwrap() })].into();
    let run = Request::Run { target: Target::Op { op: "dspk:plot".into(), params: BTreeMap::new() }, fillings: vec![filling.clone()], options: BTreeMap::new() };
    let reply = post(&s.addr, true, "application/grpc", Some(&s.control), grpc_frame(&request(&me, run.clone()))).await;
    let Ok(Answer::Started { runs, .. }) = answer(&reply.body[5..]) else { panic!("not started") };
    let record = loop {
        let watch = Request::RunWatch { run: runs[0].clone() };
        let reply = post(&s.addr, false, "application/proto", Some(&s.control), request(&me, watch)).await;
        if let Ok(Answer::Run { state: kitchen_client::RunState::Ended { record } }) = answer(&reply.body) {
            break record;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };
    assert_eq!(record.status, RunStatus::Succeeded);
    assert_eq!((record.by.as_str(), record.via.as_str()), ("ana", "notebook"));

    // gRPC-Web over HTTP/1.1: the trailers as the body's last frame
    let reply = post(&s.addr, false, "application/grpc-web+proto", Some(&s.control), grpc_frame(&request(&me, Request::Types))).await;
    assert_eq!(reply.status, StatusCode::OK);
    let len = u32::from_be_bytes(reply.body[1..5].try_into().unwrap()) as usize;
    assert!(answer(&reply.body[5..5 + len]).is_ok());
    let trailer = &reply.body[5 + len..];
    assert_eq!(trailer[0], 0x80);
    assert_eq!(std::str::from_utf8(&trailer[5..]).unwrap(), "grpc-status: 0\r\n");

    // Connect with JSON, as curl sends it: protobuf's own JSON mapping both ways
    let json = br#"{"caller": {"by": "someone", "via": "curl"}, "lineage": {"id": "sha256:00"}}"#.to_vec();
    let reply = post(&s.addr, false, "application/json", Some(&s.control), json).await;
    assert_eq!(reply.status, StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(&reply.body).unwrap();
    assert!(body["error"]["kind"].is_string(), "{body}");

    // A reader's run is refused in the answer, as over WebTransport
    let reply = post(&s.addr, false, "application/proto", Some(&s.read), request(&me, run)).await;
    assert!(matches!(answer(&reply.body), Err(RuntimeError::Forbidden(_))));
}

#[tokio::test(flavor = "multi_thread")]
async fn transport_failures_are_said_in_each_protocols_terms() {
    let s = serve("failures").await;
    let me = Caller::new("someone", "test");

    // No token: gRPC's trailers-only status 16; Connect's 401 with a JSON error
    let reply = post(&s.addr, true, "application/grpc", None, grpc_frame(&request(&me, Request::Types))).await;
    assert_eq!(reply.headers["grpc-status"], "16");
    let reply = post(&s.addr, false, "application/json", Some("not-a-token"), b"{}".to_vec()).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    let body: serde_json::Value = serde_json::from_slice(&reply.body).unwrap();
    assert_eq!(body["code"], "unauthenticated");

    // Not a message; not a protocol; not the path
    let reply = post(&s.addr, false, "application/json", Some(&s.control), b"{\"nope\": 1}".to_vec()).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    let reply = post(&s.addr, false, "text/plain", Some(&s.control), Vec::new()).await;
    assert_eq!(reply.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test(flavor = "multi_thread")]
async fn plain_http_stays_on_loopback_and_tls_comes_from_the_authority() {
    let dir = std::env::temp_dir().join(format!("kitchen-server-http-tls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let authority = Arc::new(Authority::open_or_create(&dir.join("authority")).unwrap());
    let runtime: Arc<dyn Runtime> = Arc::new(Local::new("dspk", Catalog::new([Type::new("signal")]), &dir, Arc::new(Fake), 1).unwrap());
    let everywhere = ServerOptions { bind: "0.0.0.0:0".parse().unwrap(), ..ServerOptions::loopback("test") };
    assert!(Server::bind(everywhere, authority.clone(), runtime.clone()).is_err(), "plain HTTP off loopback is refused");
    let tls = ServerOptions { http: kitchen_server::Http::Authority, ..ServerOptions::loopback("test") };
    let server = Server::bind(tls, authority, runtime).unwrap();
    assert!(server.http_url("localhost").unwrap().unwrap().starts_with("https://localhost:"));
}

/// A GET over HTTP/1.1: the status, the headers and the body.
async fn get(addr: &str, path: &str) -> (StatusCode, HeaderMap, Bytes) {
    let io = TokioIo::new(tokio::net::TcpStream::connect(addr).await.unwrap());
    let (mut sender, connection) = hyper::client::conn::http1::handshake(io).await.unwrap();
    tokio::spawn(connection);
    let request = hyper::Request::get(path).header("host", addr).body(Full::new(Bytes::new())).unwrap();
    let response = sender.send_request(request).await.unwrap();
    let (parts, body) = response.into_parts();
    (parts.status, parts.headers, body.collect().await.unwrap().to_bytes())
}

#[tokio::test(flavor = "multi_thread")]
async fn pages_are_served_isolated_and_never_from_outside_their_folder() {
    let dir = std::env::temp_dir().join(format!("kitchen-server-http-pages-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("web")).unwrap();
    std::fs::write(dir.join("web/index.html"), "<p>hi</p>").unwrap();
    std::fs::write(dir.join("web/app.wasm"), b"\0asm").unwrap();
    std::fs::write(dir.join("secret.txt"), "no").unwrap();
    let authority = Arc::new(Authority::open_or_create(&dir.join("authority")).unwrap());
    let runtime: Arc<dyn Runtime> = Arc::new(Local::new("dspk", Catalog::new([Type::new("signal")]), &dir, Arc::new(Fake), 1).unwrap());
    let options = ServerOptions { web_root: Some(dir.join("web")), ..ServerOptions::loopback("pages") };
    let server = Server::bind(options, authority, runtime).unwrap();
    let hash = server.serve_info().certificate_sha256.clone();
    let addr = server.http_url("127.0.0.1").unwrap().unwrap().trim_start_matches("http://").to_string();
    tokio::spawn(server.run());

    // Where WebTransport is, and its certificate's hash
    let (status, _, body) = get(&addr, "/kitchen-serve.json").await;
    assert_eq!(status, StatusCode::OK);
    let info: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(info["server"], "pages");
    assert_eq!(info["certificate_sha256"].as_array().unwrap().len(), hash.len());

    // The page, cross-origin isolated; the module with its type
    let (status, headers, body) = get(&addr, "/").await;
    assert_eq!((status, body.as_ref()), (StatusCode::OK, b"<p>hi</p>".as_slice()));
    assert_eq!(headers["cross-origin-opener-policy"], "same-origin");
    assert_eq!(headers["cross-origin-embedder-policy"], "require-corp");
    assert_eq!(get(&addr, "/app.wasm").await.1["content-type"], "application/wasm");

    // Nothing outside the folder
    for path in ["/../secret.txt", "/%2e%2e/secret.txt", "/web/../../secret.txt", "/missing.js"] {
        assert_eq!(get(&addr, path).await.0, StatusCode::NOT_FOUND, "{path}");
    }
}
