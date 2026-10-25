import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";

const firstOrigin = "http://127.0.0.1:19320";
const secondOrigin = "http://127.0.0.1:19321";
const reports: unknown[] = [];

// ---------------------------------------------------------
// Serve two origins so WebKit enforces real origin boundaries.
// Reports come from page assertions, not visible window counts.
// ---------------------------------------------------------
function serve(request: IncomingMessage, response: ServerResponse): void {
  const url = new URL(request.url ?? "/", firstOrigin);
  response.setHeader("Cache-Control", "no-store");
  if (url.pathname === "/results") {
    response.setHeader("Content-Type", "application/json");
    response.end(JSON.stringify(reports, null, 2));
    return;
  }
  if (url.pathname === "/report" && request.method === "POST") {
    let body = "";
    request.on("data", (chunk: Buffer) => {
      body += chunk.toString();
    });
    request.on("end", () => {
      try {
        const report: unknown = JSON.parse(body);
        reports.push(report);
        process.stdout.write(`${JSON.stringify(report)}\n`);
        response.end("ok");
      } catch {
        response.writeHead(400).end();
      }
    });
    return;
  }
  response.setHeader("Content-Type", "text/html; charset=utf-8");
  if (url.pathname === "/child") {
    response.end(`<!doctype html><title>Relationship child</title><p>Controlled child</p><script>
      addEventListener('message', async event => {
        if (event.origin !== '${firstOrigin}' || event.source !== opener) return;
        let ipcDenied = true;
        if (window.__TAURI_INTERNALS__) {
          try { await window.__TAURI_INTERNALS__.invoke('browser_navigation', {}); ipcDenied = false; }
          catch {}
        }
        opener.postMessage({ token: event.data.token, openerPresent: !!opener, ipcDenied }, event.origin);
      });
    </script>`);
    return;
  }
  response.end(`<!doctype html><title>Window relationship checks</title>
    <h1>Window relationship checks</h1><button id="run">Run checks</button><pre id="output">Ready</pre>
    <a href="/child" target="_blank">Open child link</a>
    <script>
    const output = document.getElementById('output');
    const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
    async function run() {
      const results = [];
      const token = crypto.randomUUID();
      const check = (name, passed) => results.push({ name, passed: !!passed });
      const child = window.open('/child', '', 'width=620,height=440');
      check('window.open returns a WindowProxy', child !== null);
      if (child) {
        await pause(700);
        try {
          check('same-origin opener is preserved', child.opener === window);
          child.relationshipMarker = token;
          check('same-origin script access works', child.relationshipMarker === token);
        } catch { check('same-origin script access works', false); }
      }
      if (child) {
        let nested;
        try {
          nested = child.open('/child');
          await pause(500);
          check('popup delegate preserves nested opener', nested && nested.opener === child);
        } catch { check('popup delegate preserves nested opener', false); }
        if (nested) { nested.close(); await pause(400); check('nested child closes alone', nested.closed && !child.closed); }
      }
      const cross = window.open('${secondOrigin}/child');
      check('cross-origin window returns a WindowProxy', cross !== null);
      if (cross) {
        await pause(700);
        try { void cross.document; check('cross-origin document access is denied', false); }
        catch { check('cross-origin document access is denied', true); }
        const reply = await new Promise(resolve => {
          const timeout = setTimeout(() => { removeEventListener('message', receive); resolve(null); }, 4000);
          function receive(event) {
            if (event.source !== cross || event.origin !== '${secondOrigin}' || event.data.token !== token) return;
            clearTimeout(timeout); removeEventListener('message', receive); resolve(event.data);
          }
          addEventListener('message', receive);
          cross.postMessage({ token }, '${secondOrigin}');
        });
        check('cross-origin opener postMessage round trip', reply?.openerPresent);
        check('remote popup cannot invoke browser controls', reply?.ipcDenied);
        cross.close(); await pause(600);
        check('script closes the requested cross-origin child', cross.closed);
        check('other child remains open', child && !child.closed);
      }
      if (child) { child.close(); await pause(600); check('same-origin child close is reported', child.closed); }
      const blank = window.open('about:blank');
      check('blank popup preserves scripting', blank && blank.opener === window);
      if (blank) { blank.close(); await pause(400); check('blank child closes', blank.closed); }
      for (const address of ['file:///tmp/relationship-fixture', 'tauri://localhost/', 'mailto:blocked@example.invalid']) {
        const blocked = window.open(address);
        check('blocked destination: ' + address.split(':')[0], blocked === null);
        if (blocked) blocked.close();
      }
      check('source remains open', !window.closed);
      const report = { token, source: location.href, results, passed: results.every(result => result.passed) };
      output.textContent = JSON.stringify(report, null, 2);
      await fetch('/report', { method: 'POST', body: JSON.stringify(report) });
    }
    document.getElementById('run').addEventListener('click', run);
    if (new URLSearchParams(location.search).has('autorun')) setTimeout(run, 800);
    </script>`);
}

for (const origin of [firstOrigin, secondOrigin]) {
  const url = new URL(origin);
  createServer(serve).listen(Number(url.port), url.hostname, () => {
    process.stdout.write(`Fixture listening at ${origin}\n`);
  });
}
