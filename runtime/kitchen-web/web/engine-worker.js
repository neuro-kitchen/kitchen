// The engine worker: a page that no server served runs its app's engine here, in a module worker
// of its own, so the engine can own the browser's GPU (a device belongs to the thread that made
// it) while the page draws on its own threads. Started by kitchen-web's `start_worker`:
//
//   new Worker("engine-worker.js?module=./my_app_engine.js", { type: "module" })
//
// The module (built with wasm-bindgen --target web) exports its init function as default and a
// class `Engine` with:
// - `answer(bytes)`: a `kitchen.runtime.v1.Request` in, an `Answer` out (`kitchen_web::worker::
//   answer` over the app's runtime), or a promise of it;
// - optionally `ready()`: why the engine cannot compute on the GPU, "" when it can;
// - optionally `set_files(files)`: the files the user picked;
// - optionally `set_canvas(canvas)`: an OffscreenCanvas to draw on.
// Calls are answered one at a time, in order (dsp-kitchen's engine worker does the same).
const params = new URL(self.location.href).searchParams;
const module = await import(params.get("module") || "./engine.js");
await module.default();
const engine = new module.Engine();
let why = "";
try {
  why = (engine.ready ? await engine.ready() : "") || "";
} catch (e) {
  why = String(e);
}
let queue = Promise.resolve();
self.onmessage = (e) => {
  const message = e.data;
  if (message.files) {
    engine.set_files?.(message.files);
    return;
  }
  if (message.canvas) {
    engine.set_canvas?.(message.canvas);
    return;
  }
  queue = queue.then(async () => {
    const out = await engine.answer(new Uint8Array(message.bytes));
    // A copy of its own, transferred (the module's memory may be shared)
    const copy = new Uint8Array(out);
    self.postMessage({ id: message.id, bytes: copy.buffer }, [copy.buffer]);
  });
};
self.postMessage({ ready: why });
