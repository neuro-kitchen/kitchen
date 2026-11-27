//! The server: a WebTransport endpoint answering the runtime API's calls with a
//! [`Runtime`](kitchen_client::Runtime).

use std::net::SocketAddr;
use std::sync::Arc;

use kitchen_client::{Caller, Request, Runtime, RuntimeError};
use kitchen_network::auth::{Authority, Peer};
use kitchen_network::pair::{PairReply, PairToken, ServeInfo, WebHello, WebWelcome};
use prost::Message;

use crate::{frames, ServerError, ServerResult, HELLO_TIMEOUT, MAX_REQUEST};

/// Where and as what a server serves.
#[derive(Clone, Debug)]
pub struct ServerOptions {
    /// The UDP address (port 0: any free port).
    pub bind: SocketAddr,
    /// The names the server is reached by (host names, addresses), put in its certificate and
    /// told to a pairing client. Loopback names are always added.
    pub names: Vec<String>,
    /// Its name, told to callers.
    pub server_name: String,
    /// Whether Connect, gRPC and gRPC-Web are served on TCP too, on the same port number.
    pub http: Http,
}

/// The fallback on TCP ([`crate::http`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Http {
    /// WebTransport only.
    Off,
    /// Plain HTTP (HTTP/1.1, and HTTP/2 without TLS), on a loopback address only: nothing leaves
    /// the computer.
    Plain,
    /// TLS with a certificate from the server's authority (HTTP/2 and HTTP/1.1 by ALPN): a client
    /// trusts the authority's CA (`ca.pem`), as a paired computer does.
    Authority,
}

impl ServerOptions {
    /// On this computer only, on any free port.
    pub fn loopback(server_name: impl Into<String>) -> Self {
        Self { bind: SocketAddr::from(([127, 0, 0, 1], 0)), names: Vec::new(), server_name: server_name.into(), http: Http::Plain }
    }
}

pub(crate) struct State {
    pub(crate) authority: Arc<Authority>,
    runtime: Arc<dyn Runtime>,
    names: Vec<String>,
    server_name: String,
    path: String,
}

/// The runtime API's endpoint, bound.
pub struct Server {
    tcp: Option<(tokio::net::TcpListener, Option<tokio_rustls::TlsAcceptor>)>,
    endpoint: wtransport::Endpoint<wtransport::endpoint::endpoint_side::Server>,
    info: ServeInfo,
    state: Arc<State>,
}

impl Server {
    /// Binds `options.bind` (UDP) with a new 14-day certificate, answering the callers
    /// `authority` lets in with `runtime`. Call from within a Tokio runtime.
    pub fn bind(options: ServerOptions, authority: Arc<Authority>, runtime: Arc<dyn Runtime>) -> ServerResult<Self> {
        let ServerOptions { bind, names, server_name, http } = options;
        let mut sans = names.clone();
        for loopback in ["localhost", "127.0.0.1", "::1"] {
            if !sans.iter().any(|n| n == loopback) {
                sans.push(loopback.into());
            }
        }
        if !bind.ip().is_unspecified() && !sans.contains(&bind.ip().to_string()) {
            sans.push(bind.ip().to_string());
        }
        // TCP first (its port, when any port will do), then UDP on the same port number
        let (tcp, bind) = match http {
            Http::Off => (None, bind),
            Http::Plain | Http::Authority => {
                if http == Http::Plain && !bind.ip().is_loopback() {
                    return Err(ServerError::Tls(format!("plain HTTP is served on a loopback address only, not on {bind}")));
                }
                let tls = match http {
                    Http::Authority => {
                        let (chain, key) = authority.server_certificate(&sans).map_err(|e| ServerError::Tls(e.to_string()))?;
                        let mut crypto = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                            .with_safe_default_protocol_versions()
                            .map_err(|e| ServerError::Tls(e.to_string()))?
                            .with_no_client_auth()
                            .with_single_cert(chain, key)
                            .map_err(|e| ServerError::Tls(e.to_string()))?;
                        crypto.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
                        Some(tokio_rustls::TlsAcceptor::from(Arc::new(crypto)))
                    }
                    _ => None,
                };
                let listener = std::net::TcpListener::bind(bind)?;
                listener.set_nonblocking(true)?;
                let bound = listener.local_addr()?;
                (Some((tokio::net::TcpListener::from_std(listener)?, tls)), bound)
            }
        };
        let identity = wtransport::Identity::self_signed(&sans).map_err(|e| ServerError::Tls(format!("the certificate: {e}")))?;
        let certificate_sha256 = identity.certificate_chain().as_slice().first().map(|c| c.hash().as_ref().to_vec()).unwrap_or_default();
        let config = wtransport::ServerConfig::builder().with_bind_address(bind).with_identity(identity).keep_alive_interval(Some(std::time::Duration::from_secs(5))).build();
        let endpoint = wtransport::Endpoint::server(config)?;
        let port = endpoint.local_addr()?.port();
        let info = ServeInfo { server: Some(server_name.clone()), webtransport_port: Some(port), certificate_sha256 };
        let path = kitchen_network::service().webtransport_path();
        Ok(Self { tcp, endpoint, info, state: Arc::new(State { authority, runtime, names, server_name, path }) })
    }

    /// The address it is bound to (UDP).
    pub fn local_addr(&self) -> ServerResult<SocketAddr> {
        Ok(self.endpoint.local_addr()?)
    }

    /// What a page is told before it connects: the port and the certificate's hash.
    pub fn serve_info(&self) -> &ServeInfo {
        &self.info
    }

    /// The URL of Connect, gRPC and gRPC-Web at `host` (`None` when TCP is off): the base a
    /// client is given (the method is [`crate::CALL_PATH`] under it).
    pub fn http_url(&self, host: &str) -> ServerResult<Option<String>> {
        let Some((listener, tls)) = &self.tcp else { return Ok(None) };
        let scheme = if tls.is_some() { "https" } else { "http" };
        Ok(Some(format!("{scheme}://{host}:{}", listener.local_addr()?.port())))
    }

    /// The session's URL at `host` (a name in its certificate).
    pub fn url(&self, host: &str) -> ServerResult<String> {
        Ok(format!("https://{host}:{}{}", self.local_addr()?.port(), self.state.path))
    }

    /// Serves sessions (and, when on, TCP connections) until the task is dropped.
    pub async fn run(self) {
        let Self { tcp, endpoint, state, .. } = self;
        let http = {
            let state = state.clone();
            async move {
                let Some((listener, tls)) = tcp else { return std::future::pending::<()>().await };
                loop {
                    let (stream, remote) = match listener.accept().await {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::warn!("runtime server: accept failed: {e}");
                            continue;
                        }
                    };
                    let (tls, state) = (tls.clone(), state.clone());
                    tokio::spawn(async move {
                        let service = hyper::service::service_fn(move |request| crate::http::handle(request, state.clone()));
                        let builder = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());
                        let served = match tls {
                            Some(tls) => match tls.accept(stream).await {
                                Ok(stream) => builder.serve_connection(hyper_util::rt::TokioIo::new(stream), service).await,
                                Err(e) => {
                                    tracing::debug!("runtime server: TLS with {remote} failed: {e}");
                                    return;
                                }
                            },
                            None => builder.serve_connection(hyper_util::rt::TokioIo::new(stream), service).await,
                        };
                        if let Err(e) = served {
                            tracing::debug!("runtime server: a connection with {remote} ended: {e}");
                        }
                    });
                }
            }
        };
        let webtransport = async move {
            loop {
                let incoming = endpoint.accept().await;
                let state = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = session(incoming, state).await {
                        tracing::debug!("runtime server: a session ended: {e}");
                    }
                });
            }
        };
        tokio::join!(http, webtransport);
    }
}

/// Whether a session's `Origin` is the page's own (`authority`: the `host:port` it reached). No
/// origin: not a browser (a program, a test), which still needs a token or a code. (From
/// lucem-stream.)
pub(crate) fn same_origin(origin: Option<&str>, authority: &str) -> bool {
    let Some(origin) = origin else { return true };
    let host = origin.strip_prefix("https://").or_else(|| origin.strip_prefix("http://")).unwrap_or("").trim_end_matches('/');
    if host.is_empty() {
        return false;
    }
    if host.eq_ignore_ascii_case(authority) {
        return true;
    }
    // This computer by any of its loopback names, on the same port
    let split = |a: &str| a.rsplit_once(':').map(|(h, p)| (h.trim_matches(['[', ']']).to_ascii_lowercase(), p.to_string()));
    let loopback = |h: &str| h == "localhost" || h == "::1" || h.starts_with("127.");
    matches!((split(host), split(authority)), (Some((h1, p1)), Some((h2, p2))) if p1 == p2 && loopback(&h1) && loopback(&h2))
}

async fn session(incoming: wtransport::endpoint::IncomingSession, state: Arc<State>) -> ServerResult<()> {
    let request = incoming.await.map_err(ServerError::protocol)?;
    if request.path() != state.path {
        request.not_found().await;
        return Ok(());
    }
    if !same_origin(request.origin(), request.authority()) {
        tracing::warn!("runtime server: refused a session from {:?} (not the page's origin)", request.origin());
        request.forbidden().await;
        return Ok(());
    }
    let connection = request.accept().await.map_err(ServerError::protocol)?;
    let (mut send, mut recv) = connection.accept_bi().await.map_err(ServerError::protocol)?;
    let peer = match tokio::time::timeout(HELLO_TIMEOUT, identify(&mut send, &mut recv, &state)).await {
        Ok(Ok(Some(peer))) => peer,
        Ok(Ok(None)) => {
            let _ = send.finish().await;
            connection.close(wtransport::VarInt::from_u32(1), b"not let in");
            return Err(ServerError::Refused("an unknown token or a wrong code".into()));
        }
        Ok(Err(e)) => {
            connection.close(wtransport::VarInt::from_u32(1), b"not let in");
            return Err(e);
        }
        Err(_) => {
            connection.close(wtransport::VarInt::from_u32(1), b"too slow");
            return Err(ServerError::Protocol("the caller did not say who it is".into()));
        }
    };
    let welcome = WebWelcome {
        server_name: state.server_name.clone(),
        server_version: env!("CARGO_PKG_VERSION").into(),
        peer_name: peer.name.clone(),
        permission: peer.permission,
        has_files: false,
        runtime: String::new(),
        plugins: Vec::new(),
    };
    frames::send_json(&mut send, &welcome).await?;
    let _ = send.finish().await;
    tracing::info!("runtime server: {} connected ({})", peer.name, peer.permission);
    let peer = Arc::new(peer);
    // One stream per call, answered concurrently
    loop {
        let Ok((mut send, mut recv)) = connection.accept_bi().await else { break };
        let (state, peer) = (state.clone(), peer.clone());
        tokio::spawn(async move {
            let answer = match frames::read(&mut recv, MAX_REQUEST).await {
                Ok(bytes) => answer(&state, &peer, &bytes).await,
                Err(e) => kitchen_wire::encode_answer(&Err(RuntimeError::Invalid(format!("not a request: {e}")))),
            };
            if frames::write(&mut send, &answer.encode_to_vec()).await.is_ok() {
                let _ = send.finish().await;
            }
        });
    }
    Ok(())
}

/// Who the caller is: by its token, or by pairing now (it then gets a token). `None`: not let in.
async fn identify(send: &mut wtransport::SendStream, recv: &mut wtransport::RecvStream, state: &State) -> ServerResult<Option<Peer>> {
    match frames::next_json::<WebHello>(recv).await? {
        WebHello::Token { token } => Ok(state.authority.authorize_token(&token)),
        WebHello::Pair { pair } => {
            let pairing = match state.authority.pairing_start(&pair) {
                Ok((pairing, reply)) => {
                    frames::send_json(send, &reply).await?;
                    pairing
                }
                Err(refusal) => {
                    frames::send_json(send, &refusal).await?;
                    return Ok(None);
                }
            };
            let proof: PairToken = frames::next_json(recv).await?;
            let reply = state.authority.pairing_finish_token(pairing, &proof, &state.names);
            let peer = match &reply {
                PairReply::Token { token, .. } => state.authority.authorize_token(token),
                _ => None,
            };
            frames::send_json(send, &reply).await?;
            Ok(peer)
        }
    }
}

/// Whether `peer` may make `request`: running, cancelling and exporting change things.
fn allowed(peer: &Peer, request: &Request) -> Result<(), RuntimeError> {
    let changes = matches!(request, Request::Run { .. } | Request::RunCancel { .. } | Request::ResultExport { .. });
    if changes && !peer.can_control() {
        return Err(RuntimeError::Forbidden(format!("{} may read only: running, cancelling and exporting need control", peer.name)));
    }
    Ok(())
}

/// One call's answer, made on the blocking pool.
pub(crate) async fn answer(state: &Arc<State>, peer: &Arc<Peer>, bytes: &[u8]) -> kitchen_wire::pb::Answer {
    let (caller, request) = match kitchen_wire::parse_request(bytes) {
        Ok(call) => call,
        Err(e) => return kitchen_wire::encode_answer(&Err(RuntimeError::Invalid(format!("not a request: {e}")))),
    };
    if let Err(e) = allowed(peer, &request) {
        return kitchen_wire::encode_answer(&Err(e));
    }
    // The person is the one let in; what they called through is theirs to say
    let via = if caller.via.is_empty() { "remote".to_string() } else { caller.via };
    let caller = Caller::new(peer.name.clone(), via);
    let runtime = state.runtime.clone();
    let result = tokio::task::spawn_blocking(move || runtime.call(&caller, request)).await.unwrap_or_else(|e| Err(RuntimeError::Io(format!("the call failed: {e}"))));
    kitchen_wire::encode_answer(&result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kitchen_network::pair::Permission;

    #[test]
    fn only_the_pages_own_origin_or_a_program_connects() {
        assert!(same_origin(None, "127.0.0.1:4433"));
        assert!(same_origin(Some("https://127.0.0.1:4433"), "127.0.0.1:4433"));
        assert!(same_origin(Some("http://localhost:4433"), "127.0.0.1:4433"));
        assert!(!same_origin(Some("https://evil.example"), "127.0.0.1:4433"));
        assert!(!same_origin(Some("http://localhost:9999"), "127.0.0.1:4433"));
        assert!(!same_origin(Some("null"), "127.0.0.1:4433"));
    }

    #[test]
    fn a_read_only_peer_reads_but_does_not_run() {
        let reader = Peer { name: "ben".into(), permission: Permission::Read, pairing_only: false };
        let cancel = Request::RunCancel { run: "r".into() };
        assert!(matches!(allowed(&reader, &cancel), Err(RuntimeError::Forbidden(_))));
        assert!(allowed(&reader, &Request::Types).is_ok());
        let controller = Peer { permission: Permission::Control, ..reader };
        assert!(allowed(&controller, &cancel).is_ok());
    }
}
