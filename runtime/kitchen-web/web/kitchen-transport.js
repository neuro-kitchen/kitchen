// WebTransport for kitchen-web's page link (`globalThis.kitchenTransport`), as lucem's loader does
// it: kept in JavaScript so the Rust side stays off web-sys's unstable WebTransport bindings.
// Load it before the app's module: <script src="kitchen-transport.js"></script>.
//
// Messages are a 4-byte big-endian length, then the bytes. `open(url, hash)` connects (a
// self-signed certificate is accepted by its SHA-256 only) and opens the control stream (who the
// page is, as JSON); `request(bytes)` opens a stream for one call and resolves with its answer.
(() => {
  if (!("WebTransport" in globalThis)) return;
  const frame = (bytes) => {
    const out = new Uint8Array(4 + bytes.length);
    new DataView(out.buffer).setUint32(0, bytes.length);
    out.set(bytes, 4);
    return out;
  };
  const framed = (reader) => {
    let buf = new Uint8Array(0);
    const need = async (n) => {
      while (buf.length < n) {
        const { value, done } = await reader.read();
        if (done) throw new Error("the stream closed");
        const grown = new Uint8Array(buf.length + value.length);
        grown.set(buf);
        grown.set(value, buf.length);
        buf = grown;
      }
    };
    return {
      async next() {
        await need(4);
        const len = new DataView(buf.buffer, buf.byteOffset).getUint32(0);
        await need(4 + len);
        const out = buf.slice(4, 4 + len);
        buf = buf.slice(4 + len);
        return out;
      },
    };
  };
  globalThis.kitchenTransport = {
    async open(url, hash) {
      const options = hash && hash.length ? { serverCertificateHashes: [{ algorithm: "sha-256", value: hash.slice() }] } : {};
      const wt = new WebTransport(url, options);
      await wt.ready;
      const control = await wt.createBidirectionalStream();
      const writer = control.writable.getWriter();
      const reader = framed(control.readable.getReader());
      const encoder = new TextEncoder(), decoder = new TextDecoder();
      return {
        control: {
          send: async (text) => writer.write(frame(encoder.encode(text))),
          next: async () => decoder.decode(await reader.next()),
        },
        async request(bytes) {
          const stream = await wt.createBidirectionalStream();
          const w = stream.writable.getWriter();
          await w.write(frame(bytes));
          // Our side ends here, not awaited: a server that answered at once may close the stream
          // first, and the close then "fails" although the answer is there
          w.close().catch(() => {});
          return framed(stream.readable.getReader()).next();
        },
        close() {
          wt.close();
        },
      };
    },
  };
})();
