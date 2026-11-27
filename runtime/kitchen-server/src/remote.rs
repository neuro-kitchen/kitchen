//! A server's runtime from a program: [`Runtime`] over a WebTransport session.

use std::future::Future;
use std::sync::Arc;

use kitchen_client::{Answer, Caller, Request, Runtime, RuntimeError};
use kitchen_network::pair::{ClientPairing, PairReply, PairedToken, WebHello, WebWelcome};
use prost::Message;

use crate::{frames, ServerError, ServerResult};

/// Where a server is.
#[derive(Clone, Debug)]
pub struct RemoteOptions {
    /// The session's URL (`https://host:port/kitchen`: [`crate::Server::url`]).
    pub url: String,
    /// The SHA-256 of the server's certificate ([`crate::ServeInfo::certificate_sha256`]).
    pub certificate_sha256: Vec<u8>,
}

/// A session with a server, as a [`Runtime`]: every call is a stream of its own, so calls made
/// from several threads go out together. It keeps two threads of its own for the session, so it
/// is called the same way from plain code and from async code (where it blocks the calling
/// thread until the answer comes).
pub struct Remote {
    rt: Option<tokio::runtime::Runtime>,
    connection: Arc<wtransport::Connection>,
    welcome: WebWelcome,
}

impl Remote {
    /// Connects with a token from an earlier pairing.
    pub fn connect(options: &RemoteOptions, token: &str) -> ServerResult<Self> {
        let hello = WebHello::Token { token: token.into() };
        Self::open(options, move |send, recv| Box::pin(async move { say_hello(send, recv, hello).await.map(|()| None) })).map(|(remote, _)| remote)
    }

    /// Pairs as `name` with a code the server showed (`42-8317-2956`), then connects: the
    /// session, and the token to connect with next time.
    pub fn pair(options: &RemoteOptions, code: &str, name: &str) -> ServerResult<(Self, PairedToken)> {
        let (pairing, hello) = ClientPairing::start(code, name).map_err(|e| ServerError::Refused(e.to_string()))?;
        let (remote, paired) = Self::open(options, move |send, recv| {
            Box::pin(async move {
                say_hello(send, recv, WebHello::Pair { pair: hello }).await?;
                let first: PairReply = frames::next_json(recv).await.map_err(refused)?;
                let (waiting, proof) = pairing.token(first).map_err(|e| ServerError::Refused(e.to_string()))?;
                frames::send_json(send, &proof).await?;
                let last: PairReply = frames::next_json(recv).await.map_err(refused)?;
                waiting.finish(last).map(Some).map_err(|e| ServerError::Refused(e.to_string()))
            })
        })?;
        Ok((remote, paired.ok_or_else(|| ServerError::Protocol("paired without a token".into()))?))
    }

    /// What the server said when it let this session in: its name, who it took the caller for,
    /// and what they may do.
    pub fn welcome(&self) -> &WebWelcome {
        &self.welcome
    }

    fn open<H>(options: &RemoteOptions, hello: H) -> ServerResult<(Self, Option<PairedToken>)>
    where
        H: for<'a> FnOnce(&'a mut wtransport::SendStream, &'a mut wtransport::RecvStream) -> std::pin::Pin<Box<dyn Future<Output = ServerResult<Option<PairedToken>>> + Send + 'a>> + Send + 'static,
    {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).thread_name("kitchen-remote").enable_all().build()?;
        let digest: [u8; 32] = options.certificate_sha256.as_slice().try_into().map_err(|_| ServerError::Tls("a certificate hash is 32 bytes".into()))?;
        let url = options.url.clone();
        let opened = block(&rt, async move {
            // Bound in the server's address family: a computer (or a container) without IPv6 cannot
            // bind wtransport's default dual-stack socket
            let authority = url.strip_prefix("https://").unwrap_or(&url).split('/').next().unwrap_or_default().to_string();
            let ipv4 = tokio::net::lookup_host(authority).await?.next().is_some_and(|a| a.is_ipv4());
            let builder = wtransport::ClientConfig::builder();
            let builder = if ipv4 { builder.with_bind_address(std::net::SocketAddr::from(([0, 0, 0, 0], 0))) } else { builder.with_bind_default() };
            let config = builder.with_server_certificate_hashes([wtransport::tls::Sha256Digest::new(digest)]).keep_alive_interval(Some(std::time::Duration::from_secs(5))).build();
            let endpoint = wtransport::Endpoint::client(config)?;
            let connection = endpoint.connect(url).await.map_err(ServerError::protocol)?;
            let (mut send, mut recv) = connection.open_bi().await.map_err(ServerError::protocol)?.await.map_err(ServerError::protocol)?;
            let paired = hello(&mut send, &mut recv).await?;
            let welcome: WebWelcome = frames::next_json(&mut recv).await.map_err(refused)?;
            Ok::<_, ServerError>((connection, welcome, paired))
        })?;
        let (connection, welcome, paired) = opened?;
        Ok((Self { rt: Some(rt), connection: Arc::new(connection), welcome }, paired))
    }
}

/// The server closes a session it does not let in, so a hello's answer that never comes is a
/// refusal.
fn refused(e: ServerError) -> ServerError {
    match e {
        ServerError::Protocol(why) => ServerError::Refused(why),
        other => other,
    }
}

async fn say_hello(send: &mut wtransport::SendStream, _recv: &mut wtransport::RecvStream, hello: WebHello) -> ServerResult<()> {
    frames::send_json(send, &hello).await
}

/// Runs `future` on `rt` and waits for it from any thread, async or not.
fn block<F>(rt: &tokio::runtime::Runtime, future: F) -> ServerResult<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    rt.spawn(async move {
        let _ = tx.send(future.await);
    });
    rx.recv().map_err(|_| ServerError::Protocol("the session's threads stopped".into()))
}

async fn call(connection: Arc<wtransport::Connection>, caller: Caller, request: Request) -> Result<Answer, RuntimeError> {
    let io = |e: ServerError| RuntimeError::Io(e.to_string());
    let (mut send, mut recv) = connection.open_bi().await.map_err(|e| io(ServerError::protocol(e)))?.await.map_err(|e| io(ServerError::protocol(e)))?;
    let message = kitchen_wire::encode_request(&caller, &request);
    frames::write(&mut send, &message.encode_to_vec()).await.map_err(io)?;
    let _ = send.finish().await;
    let bytes = frames::read(&mut recv, kitchen_wire::MAX_FRAME).await.map_err(io)?;
    kitchen_wire::parse_answer(&bytes).map_err(|e| RuntimeError::Io(format!("not an answer: {e}")))?
}

impl Runtime for Remote {
    fn call(&self, caller: &Caller, request: Request) -> Result<Answer, RuntimeError> {
        let rt = self.rt.as_ref().expect("the session's threads live as long as the session");
        block(rt, call(self.connection.clone(), caller.clone(), request)).map_err(|e| RuntimeError::Io(e.to_string()))?
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        self.connection.close(wtransport::VarInt::from_u32(0), b"done");
        // Not a blocking shutdown: a session may be dropped on an async thread
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}
