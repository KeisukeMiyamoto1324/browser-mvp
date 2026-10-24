import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";

let visits = 0;

// ---------------------------------------------------------
// Serve disposable pages for native navigation checks.
// Keep slow responses open until stopped or completed.
// ---------------------------------------------------------
createServer((request: IncomingMessage, response: ServerResponse): void => {
  const path = new URL(request.url ?? "/", "http://localhost").pathname;
  if (path === "/redirect") {
    response.writeHead(302, { Location: "/two" });
    response.end();
    return;
  }
  response.writeHead(200, {
    "Content-Type": "text/html; charset=utf-8",
    "Cache-Control": "no-store",
  });
  visits += 1;
  response.write(`<!doctype html><html><head><title>Navigation ${path}</title></head>
<body style="font:16px -apple-system;padding:32px;background:#fff;color:#222">
<h1>Navigation ${path}</h1><p>Request ${visits}</p>
<p><a href="/one">Page one</a> · <a href="/two">Page two</a> ·
<a href="/redirect">Redirect</a> · <a href="/slow">Slow page</a> ·
<a href="#section">Same-document anchor</a> · <a href="about:blank">Blank page</a></p>
<button onclick="history.back=()=>{};history.forward=()=>{};this.textContent='Page history methods replaced'">Replace page history methods</button>
<p><a href="/popup" target="_blank">Open new tab</a></p>
<label>Tab note <input aria-label="Tab note"></label>
<div style="height:1400px"></div><h2 id="section">Section</h2>`);
  if (path !== "/slow") {
    response.end("</body></html>");
    return;
  }
  const timer = setTimeout((): void => {
    response.end("<p>Finished</p></body></html>");
  }, 30000);
  response.on("close", (): void => {
    clearTimeout(timer);
  });
}).listen(18765, "127.0.0.1", (): void => {
  process.stdout.write("Navigation fixture: http://127.0.0.1:18765/one\n");
});
