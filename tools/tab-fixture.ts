import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";

type Submission = {
  method: string;
  body: string;
  fixture: string;
  message: string;
};
const submissions: Submission[] = [];

// ---------------------------------------------------------
// Serve disposable pin, iframe and document-state checks.
// Record actual form methods and bodies at the second host.
// ---------------------------------------------------------
createServer((request: IncomingMessage, response: ServerResponse): void => {
  const path = new URL(request.url ?? "/", "http://localhost").pathname;
  response.setHeader("Cache-Control", "no-store");
  if (path === "/submissions") {
    response.setHeader("Content-Type", "application/json");
    response.end(JSON.stringify(submissions));
    return;
  }
  response.setHeader("Content-Type", "text/html; charset=utf-8");
  if (path === "/submitted") {
    let body = "";
    request.on("data", (chunk: Buffer): void => {
      body += chunk.toString();
    });
    request.on("end", (): void => {
      const values = new URLSearchParams(body);
      const submission: Submission = {
        method: request.method ?? "",
        body,
        fixture: values.get("fixture") ?? "",
        message: values.get("message") ?? "",
      };
      submissions.push(submission);
      const result = JSON.stringify(submission, null, 2)
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;");
      response.end(
        `<!doctype html><title>Form request received</title><h1>Form request received</h1><pre>${result}</pre>`,
      );
      process.stdout.write(`${JSON.stringify(submission)}\n`);
    });
    return;
  }
  response.end(`<!doctype html><html><head><title>Tabs ${path}</title></head>
<body style="font:16px -apple-system;padding:24px;background:white;color:#222">
<h1>Tabs ${path}</h1><p><a href="/second">Same host link</a> · <a href="http://localhost:18768/external">Other host link</a></p>
<label>Document note <input aria-label="Document note"></label>
<button onclick="const f=document.createElement('iframe');f.src='http://localhost:18768/frame';f.title='Cross-origin frame';document.body.append(f)">Add other-host iframe</button>
<form action="http://localhost:18768/submitted" method="post">
<input type="hidden" name="fixture" value="preserve-post-body">
<label>Form message <input name="message" value="POST + body &amp; encoding"></label>
<button type="submit">Submit other-host POST</button></form>
<p><a href="/first">First page</a></p><div style="height:1000px"></div><h2>End</h2>
</body></html>`);
}).listen(18768, "127.0.0.1", (): void => {
  process.stdout.write("Tab fixture: http://127.0.0.1:18768/first\n");
});
