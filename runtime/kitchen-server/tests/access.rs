//! Who gets in: a computer by its client certificate (mutual TLS) on WebTransport and on TCP, and
//! not once its certificate is revoked or when another authority issued it; a page or a program
//! pairing with a code over plain HTTP, then calling with its token.

use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper_util::rt::{TokioExecutor, TokioIo};
use kitchen_client::{Answer, Caller, Job, Local, Produced, Request, Runner, Runtime, RuntimeError};
use kitchen_contract::{Catalog, Type};
use kitchen_network::auth::{Authority, ClientIdentity};
use kitchen_network::pair::{ClientPairing, PairReply, Permission};
use kitchen_server::{Http, Remote, RemoteOptions, Server, ServerOptions, CALL_PATH, PAIR_FINISH_PATH, PAIR_START_PATH};
use prost::Message;

struct Nothing;

impl Runner for Nothing {
    fn run(&self, _: &Job, _: &kitchen_client::Cancel, _: &mut dyn FnMut(usize, usize)) -> Result<Vec<Produced>, String> {
        Err("nothing to run".into())
    }
}

fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kitchen-server-access-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn runtime(dir: &std::path::Path) -> Arc<dyn Runtime> {
    Arc::new(Local::new("dspk", Catalog::new([Type::new("signal")]), dir, Arc::new(Nothing), 1).unwrap())
}

/// Serves on its own thread: the WebTransport options and the TCP base URL.
fn serve(dir: &std::path::Path, authority: Arc<Authority>, http: Http) -> (RemoteOptions, String) {
    let runtime = runtime(dir);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        rt.block_on(async move {
            let server = Server::bind(ServerOptions { http, ..ServerOptions::loopback("test") }, authority, runtime).unwrap();
            let options = RemoteOptions { url: server.url("127.0.0.1").unwrap(), certificate_sha256: server.serve_info().certificate_sha256.clone() };
            tx.send((options, server.http_url("localhost").unwrap().unwrap_or_default())).unwrap();
            server.run().await;
        });
    });
    rx.recv().unwrap()
}

fn identity(authority: &Authority, name: &str) -> ClientIdentity {
    ClientIdentity::from_pem(authority.enroll(name, Permission::Control).unwrap().pem.as_bytes()).unwrap()
}

#[test]
fn a_computer_comes_in_by_its_certificate_over_webtransport() {
    let dir = folder("mtls-wt");
    let authority = Arc::new(Authority::open_or_create(&dir.join("authority")).unwrap());
    let lab = identity(&authority, "lab-pc");
    let (options, _) = serve(&dir, authority.clone(), Http::Off);

    let remote = Remote::connect_with_certificate(&options, &lab).unwrap();
    assert_eq!(remote.welcome().peer_name, "lab-pc");
    assert_eq!(remote.welcome().permission, Permission::Control);
    // It may control: cancelling an unknown run is "not found", not "forbidden"
    let cancel = remote.call(&Caller::new("x", "test"), Request::RunCancel { run: "run-0".into() });
    assert!(matches!(cancel, Err(RuntimeError::NotFound(_))), "{cancel:?}");
    drop(remote);

    // Another authority's certificate fails the handshake; a revoked one is not let in
    let elsewhere = Authority::open_or_create(&dir.join("elsewhere")).unwrap();
    assert!(Remote::connect_with_certificate(&options, &identity(&elsewhere, "stranger")).is_err());
    authority.revoke("lab-pc").unwrap();
    assert!(Remote::connect_with_certificate(&options, &lab).is_err());
}

async fn post(stream: impl hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static, http2: bool, path: &str, content_type: &str, token: Option<&str>, body: Vec<u8>) -> (hyper::StatusCode, hyper::HeaderMap, Bytes) {
    let mut builder = hyper::Request::post(format!("https://localhost{path}")).header("content-type", content_type).header("host", "localhost");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let request = builder.body(Full::new(Bytes::from(body))).unwrap();
    let response = if http2 {
        let (mut sender, connection) = hyper::client::conn::http2::handshake(TokioExecutor::new(), stream).await.unwrap();
        tokio::spawn(connection);
        sender.send_request(request).await.unwrap()
    } else {
        let (mut sender, connection) = hyper::client::conn::http1::handshake(stream).await.unwrap();
        tokio::spawn(connection);
        sender.send_request(request).await.unwrap()
    };
    let (parts, body) = response.into_parts();
    (parts.status, parts.headers, body.collect().await.unwrap().to_bytes())
}

fn grpc(request: &Request) -> Vec<u8> {
    let message = kitchen_wire::encode_request(&Caller::new("someone", "test"), request).encode_to_vec();
    let mut out = vec![0];
    out.extend_from_slice(&(message.len() as u32).to_be_bytes());
    out.extend_from_slice(&message);
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn a_computer_comes_in_by_its_certificate_over_grpc() {
    let dir = folder("mtls-tcp");
    let authority = Arc::new(Authority::open_or_create(&dir.join("authority")).unwrap());
    let lab = identity(&authority, "lab-pc");
    let (_, base) = serve(&dir, authority.clone(), Http::Authority);
    let addr = base.trim_start_matches("https://").replace("localhost", "127.0.0.1");

    let connect = |client: Option<&ClientIdentity>| {
        let mut roots = rustls::RootCertStore::empty();
        roots.add(rustls::pki_types::CertificateDer::from(authority.ca_certificate().to_vec())).unwrap();
        let builder = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider())).with_safe_default_protocol_versions().unwrap().with_root_certificates(roots);
        let mut config = match client {
            Some(i) => builder.with_client_auth_cert(vec![i.certificate.clone().into()], rustls::pki_types::PrivateKeyDer::Pkcs8(i.private_key.clone().into())).unwrap(),
            None => builder.with_no_client_auth(),
        };
        config.alpn_protocols = vec![b"h2".to_vec()];
        let (connector, addr) = (tokio_rustls::TlsConnector::from(Arc::new(config)), addr.clone());
        async move {
            let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
            TokioIo::new(connector.connect("localhost".try_into().unwrap(), tcp).await.unwrap())
        }
    };

    // No token: the certificate says who
    let (status, headers, body) = post(connect(Some(&lab)).await, true, CALL_PATH, "application/grpc", None, grpc(&Request::RunCancel { run: "run-0".into() })).await;
    assert_eq!(status, hyper::StatusCode::OK, "{headers:?}");
    assert!(matches!(kitchen_wire::parse_answer(&body[5..]).unwrap(), Err(RuntimeError::NotFound(_))));

    // Neither a certificate nor a token
    let (_, headers, _) = post(connect(None).await, true, CALL_PATH, "application/grpc", None, grpc(&Request::Types)).await;
    assert_eq!(headers["grpc-status"], "16");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_page_pairs_over_plain_http_then_calls_with_its_token() {
    let dir = folder("pair-http");
    let authority = Arc::new(Authority::open_or_create(&dir.join("authority")).unwrap());
    let code = authority.new_pairing_code(Permission::Read).unwrap();
    let (_, base) = serve(&dir, authority.clone(), Http::Plain);
    let addr = base.trim_start_matches("http://").replace("localhost", "127.0.0.1");
    let tcp = || async { TokioIo::new(tokio::net::TcpStream::connect(addr.clone()).await.unwrap()) };

    let (pairing, hello) = ClientPairing::start(&code.display(), "tablet").unwrap();
    let (status, _, body) = post(tcp().await, false, PAIR_START_PATH, "application/json", None, serde_json::to_vec(&hello).unwrap()).await;
    assert_eq!(status, hyper::StatusCode::OK);
    let started: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let reply: PairReply = serde_json::from_value(started["reply"].clone()).unwrap();
    let (waiting, proof) = pairing.token(reply).unwrap();
    let finish = serde_json::json!({ "pairing": started["pairing"], "proof": proof });
    let (status, _, body) = post(tcp().await, false, PAIR_FINISH_PATH, "application/json", None, serde_json::to_vec(&finish).unwrap()).await;
    assert_eq!(status, hyper::StatusCode::OK);
    let paired = waiting.finish(serde_json::from_slice(&body).unwrap()).unwrap();
    assert_eq!(paired.permission, Permission::Read);

    // Its token works; the pairing cannot be finished twice
    let (_, _, body) = post(tcp().await, false, CALL_PATH, "application/proto", Some(&paired.token), kitchen_wire::encode_request(&Caller::new("x", "page"), &Request::Types).encode_to_vec()).await;
    assert!(matches!(kitchen_wire::parse_answer(&body).unwrap(), Ok(Answer::Types { .. })));
    let (status, _, _) = post(tcp().await, false, PAIR_FINISH_PATH, "application/json", None, serde_json::to_vec(&finish).unwrap()).await;
    assert_eq!(status, hyper::StatusCode::NOT_FOUND);

    // A wrong code is refused at the start
    let (_, wrong) = ClientPairing::start("00-0000-0000", "eve").unwrap();
    let (status, _, _) = post(tcp().await, false, PAIR_START_PATH, "application/json", None, serde_json::to_vec(&wrong).unwrap()).await;
    assert_eq!(status, hyper::StatusCode::FORBIDDEN);
}
