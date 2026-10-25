import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";

let visits = 0;
const port = Number(process.env.NAVIGATION_FIXTURE_PORT ?? 18765);

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
  // ---------------------------------------------------------
  // Keep content and requested window sizes explicit for Zoom.
  // Report live CSS dimensions without changing window state.
  // ---------------------------------------------------------
  if (path.startsWith("/zoom-")) {
    const contentWidth =
      path === "/zoom-narrow" ? 640 : path === "/zoom-wide" ? 1600 : 1024;
    response.end(`<!doctype html><html><head><title>Window sizing ${path}</title></head>
<body style="font:16px -apple-system;margin:0;background:#fff;color:#222">
<main style="width:${contentWidth}px;padding:24px;box-sizing:border-box;border:4px solid #3a8657">
<h1>Window sizing ${path}</h1><p>Fixed content width: ${contentWidth} CSS pixels</p>
<p id="dimensions" role="status"></p>
<p><a href="/zoom-narrow">Narrow content</a> · <a href="/zoom-normal">Normal content</a> · <a href="/zoom-wide">Wide content</a></p>
<button onclick="window.open(location.pathname,'_blank','width=700,height=500')">Open 700 × 500 window</button>
<button onclick="window.open(location.pathname,'_blank','width=1100,height=760')">Open 1100 × 760 window</button>
<button onclick="window.open(location.pathname,'_blank','width=1500,height=900')">Open 1500 × 900 window</button>
<p><label>Preserved input <input aria-label="Preserved input"></label></p>
<div style="height:1600px"></div><p>Content end</p></main>
<script>
const report = () => {
  document.getElementById('dimensions').textContent =
    'Viewport: ' + innerWidth + ' × ' + innerHeight +
    '; outer: ' + outerWidth + ' × ' + outerHeight +
    '; scale: ' + devicePixelRatio +
    '; document: ' + document.documentElement.scrollWidth + ' × ' + document.documentElement.scrollHeight;
};
addEventListener('resize', report);
report();
</script></body></html>`);
    return;
  }
  response.write(`<!doctype html><html><head><title>Navigation ${path}</title></head>
<body style="font:16px -apple-system;padding:32px;background:#fff;color:#222">
<h1>Navigation ${path}</h1><p>Request ${visits}</p>
<p><a href="/one">Page one</a> · <a href="/two">Page two</a> ·
<a href="/redirect">Redirect</a> · <a href="/slow">Slow page</a> ·
<a href="#section">Same-document anchor</a> · <a href="about:blank">Blank page</a></p>
<button onclick="history.back=()=>{};history.forward=()=>{};this.textContent='Page history methods replaced'">Replace page history methods</button>
<p><a href="/popup" target="_blank">Open new tab</a></p>
<button onclick="window.open('/window-popup','_blank','width=700,height=500')">Open popup window</button>
<label>Tab note <input aria-label="Tab note"></label>
<div style="height:1400px"></div><h2 id="section">Section</h2>`);
  if (path !== "/slow" && path !== "/slow-window") {
    response.end("</body></html>");
    return;
  }
  const timer = setTimeout(
    (): void => {
      response.end("<p>Finished</p></body></html>");
    },
    path === "/slow-window" ? 120000 : 30000,
  );
  response.on("close", (): void => {
    clearTimeout(timer);
  });
}).listen(port, "127.0.0.1", (): void => {
  process.stdout.write(`Navigation fixture: http://127.0.0.1:${port}/one\n`);
});
