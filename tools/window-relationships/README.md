# Window relationship fixture

Run `node tools/window-relationships/server.ts`, then open
`http://127.0.0.1:19320/` in the application and click **Run checks**.
The two local HTTP ports provide distinct origins. Results appear on the page,
in the server log, and at `http://127.0.0.1:19320/results`.
Use `/?autorun` to run without a click when automating the application.

The checks cover returned window references, same-origin access, cross-origin
messaging and isolation, remote IPC rejection, child-specific close, blank
popups, and blocked URL schemes. Also run from a second application window
with a different active page to check source-window ownership. The child link
allows manual testing of target links and native focus behavior.

A passing fixture does not verify native layout, shortcuts, animation, or full
named-target and navigation behavior. Compare those separately before merge.
