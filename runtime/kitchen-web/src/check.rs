//! What the browser test runs (`tests/browser/`): the page's link against a real server, over
//! WebTransport and over Connect, and against an engine worker holding an `OffscreenCanvas`. Built
//! only with the `check` feature.

use std::collections::BTreeMap;

use kitchen_client::{Answer, Caller, Reply, Request, Runtime, RuntimeError, RunState, Source, Target};
use wasm_bindgen::prelude::*;

use crate::page::{connect, serve_info, start_worker, Hello, PageLink};

fn fail(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

async fn sleep(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let global = js_sys::global();
        if let Ok(f) = js_sys::Reflect::get(&global, &"setTimeout".into()).and_then(|f| f.dyn_into::<js_sys::Function>()) {
            let _ = f.call2(&global, &resolve, &ms.into());
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

#[derive(serde::Deserialize)]
struct Options {
    code: String,
    webtransport: bool,
    /// A file on the server, run through `op` (which makes a Zarr folder).
    signal: String,
    op: String,
    /// The name this page pairs under.
    name: String,
}

/// Pairs with `options.code`, lists, runs `options.op` on `options.signal`, reads a region of its
/// output, then comes back with the token: a JSON report.
#[wasm_bindgen]
pub async fn check_server(options: String) -> Result<String, JsValue> {
    let options: Options = serde_json::from_str(&options).map_err(fail)?;
    let info = serve_info().await.map_err(fail)?.ok_or_else(|| fail("no runtime served this page"))?;
    let link = connect(&info, Hello::Code { code: options.code, name: options.name }, options.webtransport).await.map_err(fail)?;
    let me = Caller::new("whoever", "test page");
    let types = match link.call_async(&me, Request::Types).await.map_err(fail)? {
        Answer::Types { types } => types.len(),
        other => return Err(fail(format!("{other:?}"))),
    };
    let ops: Vec<String> = match link.call_async(&me, Request::Ops { filter: Default::default() }).await.map_err(fail)? {
        Answer::Ops { ops } => ops.into_iter().map(|o| o.id).collect(),
        other => return Err(fail(format!("{other:?}"))),
    };
    let filling: BTreeMap<String, Source> = [("image".into(), Source::Path { path: options.signal.into(), ty: kitchen_contract_type("image"), format: kitchen_contract_format("tiff") })].into();
    let run = Request::Run { target: Target::Op { op: options.op, params: BTreeMap::new() }, fillings: vec![filling], options: BTreeMap::new() };
    let Answer::Started { runs, .. } = link.call_async(&me, run).await.map_err(fail)? else { return Err(fail("not started")) };
    let record = loop {
        match link.call_async(&me, Request::RunWatch { run: runs[0].clone() }).await.map_err(fail)? {
            Answer::Run { state: RunState::Ended { record } } => break record,
            _ => sleep(20).await,
        }
    };
    let (output, id) = record.outputs.iter().next().ok_or_else(|| fail("no output"))?;
    let region = Request::ResultRange { id: id.clone(), array: "0".into(), start: vec![14, 30], size: vec![2, 3], compression: "lz4".into() };
    let Answer::Region { data, dtype, .. } = link.call_async(&me, region).await.map_err(fail)? else { return Err(fail("not a region")) };
    let values: Vec<u16> = data.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
    // Back with the token, as the page would be on its next visit
    let token = link.new_token().ok_or_else(|| fail("paired without a token"))?.to_string();
    let again = connect(&info, Hello::Token(token), options.webtransport).await.map_err(fail)?;
    let again_types = matches!(again.call_async(&me, Request::Types).await, Ok(Answer::Types { .. }));
    // A token nobody issued is not let in
    let stranger = connect(&info, Hello::Token("0".repeat(64)), options.webtransport).await.is_err();
    let report = serde_json::json!({
        "transport": format!("{:?}", link.transport()),
        "peer": link.welcome().peer_name,
        "permission": link.welcome().permission,
        "types": types,
        "ops": ops,
        "status": record.status,
        "by": record.by,
        "via": record.via,
        "output": output,
        "dtype": dtype,
        "region": values,
        "token_again": again_types,
        "stranger_refused": stranger,
    });
    Ok(report.to_string())
}

fn kitchen_contract_type(name: &str) -> kitchen_contract::Type {
    kitchen_contract::Type::new(name)
}

fn kitchen_contract_format(name: &str) -> kitchen_contract::Format {
    kitchen_contract::Format::parse(name).expect("a format")
}

/// Starts `script` (the engine worker, loading this module's [`Engine`]), hands it `canvas` (an
/// OffscreenCanvas) and calls it: a JSON report.
#[wasm_bindgen]
pub async fn check_worker(script: String, canvas: JsValue) -> Result<String, JsValue> {
    let (link, why): (std::sync::Arc<PageLink>, String) = start_worker(&script).await.map_err(fail)?;
    link.set_canvas(canvas).map_err(fail)?;
    let me = Caller::new("you", "page");
    let Answer::Welcome { capabilities, apps, .. } = link.call_async(&me, Request::Hello { contract: "0.1".into(), capabilities: vec![] }).await.map_err(fail)? else {
        return Err(fail("not a welcome"));
    };
    let refused = matches!(link.call_async(&me, Request::RunCancel { run: "r".into() }).await, Err(RuntimeError::Unsupported(_)));
    let server_only = link.set_files(JsValue::NULL).is_ok();
    Ok(serde_json::json!({ "transport": format!("{:?}", link.transport()), "why": why, "capabilities": capabilities, "apps": apps, "refused": refused, "files": server_only }).to_string())
}

/// The check's engine, run by `engine-worker.js`: answers `hello` with the canvas it was given.
#[wasm_bindgen]
pub struct Engine {
    canvas: std::cell::RefCell<Option<(u32, u32)>>,
}

struct Answers<'a>(&'a Engine);

impl Runtime for Answers<'_> {
    fn call(&self, caller: &Caller, request: Request) -> Reply<Answer> {
        match request {
            Request::Hello { .. } => {
                let canvas = self.0.canvas.borrow().map(|(w, h)| format!("canvas:{w}x{h}")).into_iter().collect();
                Ok(Answer::Welcome { contract: "0.1".into(), capabilities: canvas, apps: vec![caller.by.clone()] })
            }
            _ => Err(RuntimeError::Unsupported("the check's engine only says hello".into())),
        }
    }
}

// SAFETY: the engine lives on its worker's one thread (a build without threads).
unsafe impl Send for Answers<'_> {}
unsafe impl Sync for Answers<'_> {}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Engine {
        Engine { canvas: std::cell::RefCell::new(None) }
    }

    pub fn ready(&self) -> String {
        String::new()
    }

    pub fn set_canvas(&self, canvas: JsValue) {
        let size = |k: &str| js_sys::Reflect::get(&canvas, &k.into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0) as u32;
        *self.canvas.borrow_mut() = Some((size("width"), size("height")));
    }

    pub fn answer(&self, bytes: &[u8]) -> Vec<u8> {
        crate::worker::answer(&Answers(self), bytes)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
