# Tauri + React + TypeScript

This browser uses React, TypeScript, and Vite for the toolbar and Tauri 2 with Rust for a native content WebView.

The toolbar provides Back, Forward, and an address/search field. Press Enter to open a URL or search DuckDuckGo. Links that request a new window open in the same WebView.

## Development

Install dependencies with `npm install`, then start the desktop app with `npm run tauri dev`.

Use `npm run dev` for the toolbar only. Browsing needs the Tauri desktop app. The content WebView uses Tauri's `unstable` multi-WebView feature. Remote websites have no Tauri permissions.

## Checks and builds

Run `npm run format`, `npm run lint`, and `npm run check` for formatting and linting.

Run `npm run build` to check TypeScript types and build the frontend into `dist`.

Run `npm run tauri build` to build the desktop app. Rust and the macOS Xcode Command Line Tools are required.

## Apple browser passkey application

Apple's [browser passkey entitlement requirements](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.web-browser.public-key-credential) are the source for this minimum browser implementation. Apple decides whether to grant the entitlement; these features do not guarantee approval.

| App requirement | Implementation |
| --- | --- |
| Declare HTTP and HTTPS support | `src-tauri/Info.plist` registers both schemes with the Viewer role. |
| Provide a URL field, search, or bookmarks on launch | The React toolbar provides a URL field and DuckDuckGo search. Bookmarks are not required when these are available. |
| Open the requested URL and render its content | A native WKWebView loads HTTP and HTTPS directly. The macOS `Opened` event queues URLs until the local toolbar is ready and opens later URLs in the running window. |
| Preserve the destination and site content | Explicit web URLs keep their path, query, and fragment. Only non-address input becomes a search. No scripts replace site content. Back/Forward uses the page's history API. |
| Support WebAuthn for web content | The content WebView uses WebKit's built-in WebAuthn implementation. The request form also requires integration with iCloud Keychain for both registration and authentication after Apple grants the entitlement. That flow requires a separately approved, signed build. |

The app uses one content WebView. If macOS sends several web URLs at once, the last one is displayed. Non-web URLs are ignored. Opening an external URL brings the browser window forward without changing the system's default browser.

`NSAllowsArbitraryLoadsInWebContent` permits browsing HTTP sites in WKWebView. It applies only to web content, not URLSession. This is needed for a general web browser that displays third-party HTTP sites; provide that justification if submitting to the App Store. See [Apple's web-content ATS exception](https://developer.apple.com/documentation/bundleresources/information-property-list/nsapptransportsecurity/nsallowsarbitraryloadsinwebcontent).

### Verify a bundled app

Build a local test bundle with `npm run tauri build -- --debug --bundles app --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'`. This uses ad-hoc signing for local testing, not an approved distribution signature.

Inspect its URL registration with `/usr/libexec/PlistBuddy -c 'Print :CFBundleURLTypes' src-tauri/target/debug/bundle/macos/browser.app/Contents/Info.plist`.

With the app closed, run `/usr/bin/open -a "$PWD/src-tauri/target/debug/bundle/macos/browser.app" 'https://tauri.app/start/?source=browser-review#content'` to check URL delivery during launch. Repeat with the app running to check delivery to the existing window. Check that the destination appears in the content WebView and the address field. Use an HTTP site as well, and verify that the site's own redirects are reflected in the address field. Test Back, Forward, and keyword search from the toolbar.

The `open` command above is a manual OS integration check; Rust tests cover URL classification, rejection of non-web schemes, and preservation of complete web addresses. `cargo test` does not prove LaunchServices delivery.

Local verification on macOS on 2026-10-24 covered ordinary launch, cold launch from external HTTP and HTTPS URLs, URL delivery while running, server redirects, search, and Back/Forward. The toolbar remained available, and the URL field matched the content destination. A localhost test page reported `PublicKeyCredential`, `navigator.credentials.create`, and `navigator.credentials.get` as available in a secure context. These API checks do not prove successful passkey registration or authentication.

### Request and enable the entitlement

Submit the app's bundle identifier, `com.keisukemiyamoto.browser`, and accurate app details through [macOS Browsers Passkeys](https://developer.apple.com/contact/request/macos-browsers-passkeys/). Use the actual build and the requirement mapping above when explaining its browser behavior. The account eligibility requirement remains Apple's responsibility to evaluate and is outside this implementation.

After approval, configure the granted `com.apple.developer.web-browser.public-key-credential` entitlement, the appropriate provisioning profile, and a signing identity for the same team and bundle identifier. Build and sign the app again. Do not treat an entitlement key added to an ad-hoc build as an Apple grant. Tauri's [macOS bundle documentation](https://v2.tauri.app/distribute/macos-application-bundle/) explains how to supply entitlements.

WKWebView handles web authentication challenges through WebKit. Follow Apple's [passkey use in web browsers](https://developer.apple.com/documentation/authenticationservices/passkey-use-in-web-browsers) guidance for user authorization and verify the native passkey flow using the approved, signed app. A native passkey permission or successful sign-in has not been verified by the local build checks. Safari-style password AutoFill is a separate feature.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
