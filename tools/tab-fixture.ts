import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";

// ---------------------------------------------------------
// Serve disposable pin, iframe and document-state checks.
// Use two hostnames on the same local test server.
// ---------------------------------------------------------
createServer((request: IncomingMessage, response: ServerResponse): void => {
  const path = new URL(request.url ?? "/", "http://localhost").pathname;
  response.writeHead(200, {
    "Content-Type": "text/html; charset=utf-8",
    "Cache-Control": "no-store",
  });
  response.end(`<!doctype html><html><head><title>Tabs ${path}</title></head>
<body style="font:16px -apple-system;padding:24px;background:white;color:#222">
<h1>Tabs ${path}</h1><p><a href="/second">Same host link</a> · <a href="http://localhost:18768/external">Other host link</a></p>
<label>Document note <input aria-label="Document note"></label>
<button onclick="const f=document.createElement('iframe');f.src='http://localhost:18768/frame';f.title='Cross-origin frame';document.body.append(f)">Add other-host iframe</button>
<p><a href="/first">First page</a></p><div style="height:1000px"></div><h2>End</h2>
</body></html>`);
}).listen(18768, "127.0.0.1", (): void => {
  process.stdout.write("Tab fixture: http://127.0.0.1:18768/first\n");
});
