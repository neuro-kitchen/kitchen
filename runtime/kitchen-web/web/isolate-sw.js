// Makes the page cross-origin isolated where the host cannot send the headers (GitHub Pages):
// every response of this origin gets Cross-Origin-Opener-Policy and -Embedder-Policy, which a
// page needs for a build with threads (shared memory). The page registers it and reloads once so
// the page itself comes through it. (From lucem.) A kitchen-server web root sends the headers
// itself.
self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));
self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.cache === "only-if-cached" && request.mode !== "same-origin") return;
  event.respondWith(
    fetch(request).then((response) => {
      if (response.status === 0) return response;
      const headers = new Headers(response.headers);
      headers.set("Cross-Origin-Embedder-Policy", "require-corp");
      headers.set("Cross-Origin-Opener-Policy", "same-origin");
      return new Response(response.body, { status: response.status, statusText: response.statusText, headers });
    }),
  );
});
