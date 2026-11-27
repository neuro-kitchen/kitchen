// Drives the test page in headless Chromium (Playwright): `node run.mjs <page url> <query>`.
// Prints the page's report and exits non-zero when a check failed.
// Playwright as installed globally (CommonJS resolution, NODE_PATH)
import { createRequire } from "node:module";
const { chromium } = createRequire(import.meta.url)("playwright");

const [url, query] = process.argv.slice(2);
const browser = await chromium.launch();
const page = await browser.newPage();
page.on("console", (m) => console.error(`page: ${m.text()}`));
page.on("pageerror", (e) => console.error(`page error: ${e}`));
await page.goto(`${url}/index.html?${query}`);
await page.waitForFunction(() => document.title === "done", null, { timeout: 60000 });
const r = await page.evaluate(() => window.result);
await browser.close();
console.log(JSON.stringify(r, null, 2));

const problems = [];
const expect = (ok, what) => ok || problems.push(what);
for (const [name, transport, peer] of [["webtransport", "WebTransport", "page-wt"], ["connect", "Connect", "page-connect"]]) {
  const s = r[name] || {};
  expect(!s.error, `${name}: ${s.error}`);
  expect(s.transport === transport, `${name}: carried by ${s.transport}`);
  expect(s.peer === peer && s.by === peer, `${name}: recorded as ${s.by}`);
  expect(JSON.stringify(s.status).includes("succeeded"), `${name}: run ${JSON.stringify(s.status)}`);
  expect(JSON.stringify(s.region) === "[1430,1431,1432,1530,1531,1532]", `${name}: region ${s.region}`);
  expect(s.token_again === true, `${name}: the token did not work again`);
  expect(s.stranger_refused === true, `${name}: a stranger's token was let in`);
}
const w = r.worker || {};
expect(!w.error, `worker: ${w.error}`);
expect(w.transport === "Worker" && w.capabilities?.[0] === "canvas:320x200", `worker: ${JSON.stringify(w)}`);
expect(w.refused === true && w.files === true, `worker: ${JSON.stringify(w)}`);
if (problems.length) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.error("browser check: all passed");
