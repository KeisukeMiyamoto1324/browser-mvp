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

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
