//! A page's link to a runtime: the server that served it (WebTransport, else Connect over
//! `fetch`), or its own engine worker. Every call is the runtime API's protobuf, as on any other
//! transport; the link is a [`Runtime`].
//!
//! - **WebTransport** goes through `web/kitchen-transport.js` (`globalThis.kitchenTransport`):
//!   `open(url, hash)` gives a session with a control stream (who the page is, JSON) and
//!   `request(bytes)`, a stream per call. That keeps this crate off web-sys's unstable
//!   WebTransport bindings, as lucem does.
//! - **Connect** is the fallback where UDP does not get through: each call a `fetch` to
//!   `POST /kitchen.runtime.v1.Runtime/Call` with the page's token; pairing is two requests.
//! - **Threads.** The link belongs to the page's main thread, which never waits: calls made there
//!   are awaited ([`PageLink::call_async`]). A worker of a build with threads (a tile being read
//!   for the canvas) waits for its answer through [`Runtime::call`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};

use futures_util::StreamExt as _;
use kitchen_client::{Answer, Caller, Reply, Request, Runtime, RuntimeError};
use kitchen_network::pair::{ClientPairing, PairReply, Permission, ServeInfo, WebHello, WebWelcome};
use prost::Message as _;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast as _, JsValue};

const CLOSED: &str = "the connection to the runtime closed";

/// How the page says who it is.
#[derive(Clone, Debug)]
pub enum Hello {
    /// A token from an earlier pairing.
    Token(String),
    /// A pairing code, and the name the server will know this page by.
    Code {
        /// The code the server shows (`42-8317-2956`).
        code: String,
        /// This page's name.
        name: String,
    },
}

/// What carries the page's calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// WebTransport over HTTP/3: a stream per call.
    WebTransport,
    /// Connect over `fetch` (HTTP/1.1 or HTTP/2): a request per call.
    Connect,
    /// The page's own engine worker (no server).
    Worker,
}

/// An encoded answer, or why there is none.
type Raw = Result<Vec<u8>, String>;

/// Locks without waiting on the page's main thread (which may not wait): held for an instant.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    loop {
        match mutex.try_lock() {
            Ok(guard) => return guard,
            Err(std::sync::TryLockError::Poisoned(e)) => return e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => std::hint::spin_loop(),
        }
    }
}

#[derive(Default)]
struct Slot {
    answer: Mutex<Option<Raw>>,
    arrived: Condvar,
}

enum Waiter {
    Task(futures_channel::oneshot::Sender<Raw>),
    Thread(Arc<Slot>),
}

impl Waiter {
    fn deliver(self, answer: Raw) {
        match self {
            Waiter::Task(tx) => {
                let _ = tx.send(answer);
            }
            Waiter::Thread(slot) => {
                *lock(&slot.answer) = Some(answer);
                slot.arrived.notify_all();
            }
        }
    }
}

/// What the page hands its engine worker: browser objects, posted from the main thread only.
enum ToWorker {
    Files(JsValue),
    Canvas(JsValue),
}

/// Browser objects carried to the main thread through a channel.
struct OnMain<T>(T);

// SAFETY: made on the page's main thread and consumed there; the channel only carries them.
unsafe impl<T> Send for OnMain<T> {}

/// The page's link, once it is let in.
pub struct PageLink {
    /// Calls for the main thread to send: an id and the encoded request.
    out: futures_channel::mpsc::UnboundedSender<(u32, Vec<u8>)>,
    pending: Mutex<HashMap<u32, Waiter>>,
    next: AtomicU32,
    welcome: WebWelcome,
    transport: Transport,
    /// The token the page got by pairing now, to keep for next time.
    new_token: Option<String>,
    worker: Option<futures_channel::mpsc::UnboundedSender<OnMain<ToWorker>>>,
}

impl PageLink {
    fn new(welcome: WebWelcome, transport: Transport, new_token: Option<String>) -> (Arc<Self>, futures_channel::mpsc::UnboundedReceiver<(u32, Vec<u8>)>) {
        let (out, requests) = futures_channel::mpsc::unbounded();
        (Arc::new(Self { out, pending: Mutex::default(), next: AtomicU32::new(1), welcome, transport, new_token, worker: None }), requests)
    }

    /// Who the runtime takes the page for, and what it may do.
    pub fn welcome(&self) -> &WebWelcome {
        &self.welcome
    }

    /// What carries the calls.
    pub fn transport(&self) -> Transport {
        self.transport
    }

    /// The token the page got by pairing, to keep (where is the page's business).
    pub fn new_token(&self) -> Option<&str> {
        self.new_token.as_deref()
    }

    fn send(&self, bytes: Vec<u8>, waiter: Waiter) -> Result<(), RuntimeError> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        lock(&self.pending).insert(id, waiter);
        self.out.unbounded_send((id, bytes)).map_err(|_| RuntimeError::Io(CLOSED.into()))
    }

    fn deliver(&self, id: u32, answer: Raw) {
        let waiting = lock(&self.pending).remove(&id);
        if let Some(w) = waiting {
            w.deliver(answer);
        }
    }

    fn closed(&self) {
        let waiting: Vec<Waiter> = lock(&self.pending).drain().map(|(_, w)| w).collect();
        for w in waiting {
            w.deliver(Err(CLOSED.into()));
        }
        self.out.close_channel();
    }

    /// Makes a call without waiting (on the page's main thread, or any task).
    pub async fn call_async(&self, caller: &Caller, request: Request) -> Reply<Answer> {
        let (tx, rx) = futures_channel::oneshot::channel();
        self.send(kitchen_wire::encode_request(caller, &request).encode_to_vec(), Waiter::Task(tx))?;
        decode(rx.await.map_err(|_| RuntimeError::Io(CLOSED.into()))?)
    }

    /// Hands the engine worker the files the user picked (`File`s, or `[{path, file}]`).
    pub fn set_files(&self, files: JsValue) -> Result<(), RuntimeError> {
        self.to_worker(ToWorker::Files(files))
    }

    /// Hands the engine worker an `OffscreenCanvas` (from `canvas.transferControlToOffscreen()`)
    /// to draw on with the GPU it owns.
    pub fn set_canvas(&self, canvas: JsValue) -> Result<(), RuntimeError> {
        self.to_worker(ToWorker::Canvas(canvas))
    }

    fn to_worker(&self, message: ToWorker) -> Result<(), RuntimeError> {
        let tx = self.worker.as_ref().ok_or_else(|| RuntimeError::Unsupported("files and canvases go to the page's engine worker, not to a server".into()))?;
        tx.unbounded_send(OnMain(message)).map_err(|_| RuntimeError::Io("the engine worker stopped".into()))
    }
}

fn decode(raw: Raw) -> Reply<Answer> {
    let bytes = raw.map_err(RuntimeError::Io)?;
    kitchen_wire::parse_answer(&bytes).map_err(|e| RuntimeError::Io(format!("the answer could not be read: {e}")))?
}

impl Runtime for PageLink {
    /// Waits for the answer: from a worker of a build with threads only (the page's main thread
    /// may not wait, and a build without threads has no workers to wait in).
    fn call(&self, caller: &Caller, request: Request) -> Reply<Answer> {
        if cfg!(not(target_feature = "atomics")) {
            return Err(RuntimeError::Unsupported("a page waits for answers only in a worker of a build with threads: await call_async".into()));
        }
        let slot = Arc::new(Slot::default());
        self.send(kitchen_wire::encode_request(caller, &request).encode_to_vec(), Waiter::Thread(slot.clone()))?;
        let mut answer = slot.answer.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(answer) = answer.take() {
                return decode(answer);
            }
            answer = slot.arrived.wait(answer).unwrap_or_else(|e| e.into_inner());
        }
    }
}

// ---- JavaScript ---------------------------------------------------------------------------------

fn js(e: JsValue) -> String {
    e.as_string().or_else(|| js_sys::Reflect::get(&e, &"message".into()).ok().and_then(|m| m.as_string())).unwrap_or_else(|| format!("{e:?}"))
}

fn get(object: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(object, &key.into()).unwrap_or(JsValue::UNDEFINED)
}

/// Calls `object.name(args…)` and awaits the promise it returns.
async fn call(object: &JsValue, name: &str, args: &[JsValue]) -> Result<JsValue, String> {
    let f: js_sys::Function = get(object, name).dyn_into().map_err(|_| format!("{name} is not a function"))?;
    let array: js_sys::Array = args.iter().collect();
    let value = f.apply(object, &array).map_err(js)?;
    match value.dyn_into::<js_sys::Promise>() {
        Ok(promise) => wasm_bindgen_futures::JsFuture::from(promise).await.map_err(js),
        Err(value) => Ok(value),
    }
}

/// A copy outside the module's memory (a stream or a transfer refuses shared bytes).
fn copy(bytes: &[u8]) -> js_sys::Uint8Array {
    let out = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    out.copy_from(bytes);
    out
}

/// `console.log`, for what a status line would cut.
pub fn log(text: &str) {
    let console = get(&js_sys::global(), "console");
    if let Ok(f) = get(&console, "log").dyn_into::<js_sys::Function>() {
        let _ = f.call1(&console, &text.into());
    }
}

/// `fetch(url, {method: "POST", headers, body})`: the status and the body's bytes.
async fn post(url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<(u16, Vec<u8>), String> {
    let init = js_sys::Object::new();
    let h = js_sys::Object::new();
    for (k, v) in headers {
        let _ = js_sys::Reflect::set(&h, &(*k).into(), &(*v).into());
    }
    let _ = js_sys::Reflect::set(&init, &"method".into(), &"POST".into());
    let _ = js_sys::Reflect::set(&init, &"headers".into(), &h);
    let _ = js_sys::Reflect::set(&init, &"body".into(), &copy(body));
    let response = call(&js_sys::global(), "fetch", &[url.into(), init.into()]).await?;
    let status = get(&response, "status").as_f64().unwrap_or(0.0) as u16;
    let buffer = call(&response, "arrayBuffer", &[]).await?;
    Ok((status, js_sys::Uint8Array::new(&buffer).to_vec()))
}

/// The page's origin (`https://host:port`) and host name.
fn origin() -> Result<(String, String), String> {
    let location = get(&js_sys::global(), "location");
    let origin = get(&location, "origin").as_string().ok_or("no location")?;
    let hostname = get(&location, "hostname").as_string().unwrap_or_default();
    Ok((origin, hostname))
}

/// What the page's server says before the page connects (`/kitchen-serve.json`), or `None` when
/// no runtime served the page (a plain file server, GitHub Pages: the page runs alone).
pub async fn serve_info() -> Result<Option<ServeInfo>, String> {
    let (origin, _) = origin()?;
    let url = format!("{origin}{}", kitchen_network::service().serve_info_path());
    let Ok(response) = call(&js_sys::global(), "fetch", &[url.into()]).await else { return Ok(None) };
    if !get(&response, "ok").as_bool().unwrap_or(false) {
        return Ok(None);
    }
    let text = call(&response, "text", &[]).await?.as_string().unwrap_or_default();
    let info: ServeInfo = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(info.server.is_some().then_some(info))
}

// ---- connecting ---------------------------------------------------------------------------------

/// Connects to the runtime that served the page (on its main thread): WebTransport when
/// `webtransport` is allowed and the browser and the network let it through, else Connect.
pub async fn connect(info: &ServeInfo, hello: Hello, webtransport: bool) -> Result<Arc<PageLink>, String> {
    let transport = get(&js_sys::global(), "kitchenTransport");
    if let (true, false, Some(port)) = (webtransport, transport.is_undefined(), info.webtransport_port) {
        let (_, hostname) = origin()?;
        // `localhost` resolves to ::1 first, and QUIC does not fall back to IPv4 as TCP does
        let hostname = if hostname.eq_ignore_ascii_case("localhost") { "127.0.0.1".to_string() } else { hostname };
        let url = format!("https://{hostname}:{port}{}", kitchen_network::service().webtransport_path());
        match call(&transport, "open", &[url.into(), copy(&info.certificate_sha256).into()]).await {
            Ok(session) => return over_webtransport(session, hello).await,
            Err(e) => log(&format!("kitchen: WebTransport did not connect ({e}); Connect instead")),
        }
    }
    over_connect(info, hello).await
}

async fn over_webtransport(session: JsValue, hello: Hello) -> Result<Arc<PageLink>, String> {
    let control = get(&session, "control");
    let send = |text: String| {
        let control = control.clone();
        async move { call(&control, "send", &[text.into()]).await.map(|_| ()) }
    };
    let next = || {
        let control = control.clone();
        async move { call(&control, "next", &[]).await?.as_string().ok_or_else(|| "the server closed the connection".to_string()) }
    };
    let json = |e: serde_json::Error| e.to_string();
    let mut new_token = None;
    match hello {
        Hello::Token(token) => send(serde_json::to_string(&WebHello::Token { token }).map_err(json)?).await?,
        Hello::Code { code, name } => {
            let (pairing, hello) = ClientPairing::start(&code, &name).map_err(|e| e.to_string())?;
            send(serde_json::to_string(&WebHello::Pair { pair: hello }).map_err(json)?).await?;
            let reply: PairReply = serde_json::from_str(&next().await?).map_err(json)?;
            let (waiting, proof) = pairing.token(reply).map_err(|e| e.to_string())?;
            send(serde_json::to_string(&proof).map_err(json)?).await?;
            let reply: PairReply = serde_json::from_str(&next().await?).map_err(json)?;
            new_token = Some(waiting.finish(reply).map_err(|e| e.to_string())?.token);
        }
    }
    let welcome: WebWelcome = serde_json::from_str(&next().await.map_err(|_| "the server did not let the page in".to_string())?).map_err(json)?;
    let (link, mut requests) = PageLink::new(welcome, Transport::WebTransport, new_token);
    // Each call on a stream of its own, answered as soon as it is (any order)
    let sender = link.clone();
    wasm_bindgen_futures::spawn_local(async move {
        while let Some((id, bytes)) = requests.next().await {
            let (link, session) = (sender.clone(), session.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let answer = call(&session, "request", &[copy(&bytes).into()]).await.map(|reply| js_sys::Uint8Array::new(&reply).to_vec());
                link.deliver(id, answer);
            });
        }
        sender.closed();
    });
    Ok(link)
}

#[derive(serde::Deserialize)]
struct Started {
    pairing: String,
    reply: PairReply,
}

async fn over_connect(info: &ServeInfo, hello: Hello) -> Result<Arc<PageLink>, String> {
    let (origin, _) = origin()?;
    let json = |e: serde_json::Error| e.to_string();
    let (token, new) = match hello {
        Hello::Token(token) => (token, false),
        Hello::Code { code, name } => {
            let (pairing, hello) = ClientPairing::start(&code, &name).map_err(|e| e.to_string())?;
            let (_, body) = post(&format!("{origin}/kitchen.runtime.v1.Pairing/Start"), &[("content-type", "application/json")], &serde_json::to_vec(&hello).map_err(json)?).await?;
            let started: Started = serde_json::from_slice(&body).map_err(|_| String::from_utf8_lossy(&body).into_owned())?;
            let (waiting, proof) = pairing.token(started.reply).map_err(|e| e.to_string())?;
            let finish = serde_json::json!({ "pairing": started.pairing, "proof": proof });
            let (_, body) = post(&format!("{origin}/kitchen.runtime.v1.Pairing/Finish"), &[("content-type", "application/json")], &serde_json::to_vec(&finish).map_err(json)?).await?;
            let reply: PairReply = serde_json::from_slice(&body).map_err(json)?;
            (waiting.finish(reply).map_err(|e| e.to_string())?.token, true)
        }
    };
    let bearer = format!("Bearer {token}");
    let (status, body) = post(&format!("{origin}/kitchen.runtime.v1.Session/Welcome"), &[("authorization", bearer.as_str())], &[]).await?;
    if status != 200 {
        return Err(format!("not let in: {}", String::from_utf8_lossy(&body)));
    }
    let mut welcome: WebWelcome = serde_json::from_slice(&body).map_err(json)?;
    if welcome.server_name.is_empty() {
        welcome.server_name = info.server.clone().unwrap_or_default();
    }
    let (link, mut requests) = PageLink::new(welcome, Transport::Connect, new.then(|| token.clone()));
    let sender = link.clone();
    let url = format!("{origin}{}", "/kitchen.runtime.v1.Runtime/Call");
    wasm_bindgen_futures::spawn_local(async move {
        while let Some((id, bytes)) = requests.next().await {
            let (link, url, bearer) = (sender.clone(), url.clone(), bearer.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let answer = match post(&url, &[("content-type", "application/proto"), ("authorization", bearer.as_str())], &bytes).await {
                    Ok((200, body)) => Ok(body),
                    Ok((status, body)) => Err(format!("{status}: {}", String::from_utf8_lossy(&body))),
                    Err(e) => Err(e),
                };
                link.deliver(id, answer);
            });
        }
        sender.closed();
    });
    Ok(link)
}

/// Starts the page's own engine worker (`script`: `engine-worker.js`, with `?module=` naming the
/// app's engine module) for a page no server served. Returns the link, and why the engine cannot
/// compute on the GPU ("" when it can).
pub async fn start_worker(script: &str) -> Result<(Arc<PageLink>, String), String> {
    let options = web_sys::WorkerOptions::new();
    options.set_type(web_sys::WorkerType::Module);
    let worker = web_sys::Worker::new_with_options(script, &options).map_err(js)?;
    let (ready_tx, mut ready) = futures_channel::mpsc::unbounded::<String>();
    let cell: Arc<OnceLock<Arc<PageLink>>> = Arc::default();
    let to = cell.clone();
    let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
        let data = e.data();
        if let Some(why) = get(&data, "ready").as_string() {
            let _ = ready_tx.unbounded_send(why);
            return;
        }
        let (Some(id), Ok(buffer)) = (get(&data, "id").as_f64(), get(&data, "bytes").dyn_into::<js_sys::ArrayBuffer>()) else { return };
        if let Some(link) = to.get() {
            link.deliver(id as u32, Ok(js_sys::Uint8Array::new(&buffer).to_vec()));
        }
    });
    worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();
    let why = ready.next().await.ok_or("the engine worker did not start")?;
    let welcome = WebWelcome { server_name: "this browser".into(), server_version: String::new(), peer_name: "you".into(), permission: Permission::Control, has_files: true, runtime: String::new(), plugins: Vec::new() };
    let (out, mut requests) = futures_channel::mpsc::unbounded();
    let (files_tx, mut handed) = futures_channel::mpsc::unbounded::<OnMain<ToWorker>>();
    let link = Arc::new(PageLink { out, pending: Mutex::default(), next: AtomicU32::new(1), welcome, transport: Transport::Worker, new_token: None, worker: Some(files_tx) });
    let _ = cell.set(link.clone());
    // The one sender, on the main thread: calls copied out of the module's memory and transferred
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            futures_util::select! {
                r = requests.next() => {
                    let Some((id, bytes)) = r else { break };
                    let bytes = copy(&bytes);
                    let message = js_sys::Object::new();
                    let _ = js_sys::Reflect::set(&message, &"id".into(), &JsValue::from(id));
                    let _ = js_sys::Reflect::set(&message, &"bytes".into(), &bytes.buffer());
                    let _ = worker.post_message_with_transfer(&message, &js_sys::Array::of1(&bytes.buffer()));
                }
                h = handed.next() => {
                    let Some(OnMain(message)) = h else { break };
                    let out = js_sys::Object::new();
                    match message {
                        ToWorker::Files(files) => {
                            let _ = js_sys::Reflect::set(&out, &"files".into(), &files);
                            let _ = worker.post_message(&out);
                        }
                        ToWorker::Canvas(canvas) => {
                            let _ = js_sys::Reflect::set(&out, &"canvas".into(), &canvas);
                            let _ = worker.post_message_with_transfer(&out, &js_sys::Array::of1(&canvas));
                        }
                    }
                }
            }
        }
    });
    Ok((link, why))
}
