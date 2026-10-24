# Tauri + React + TypeScript

This app uses React, TypeScript, and Vite for the frontend and Tauri 2 with Rust for native commands.

## Development

Install dependencies with `npm install`, then start the desktop app with `npm run tauri dev`.

Use `npm run dev` for the frontend only. The greeting command needs the Tauri desktop app.

## Checks and builds

Run `npm run format`, `npm run lint`, and `npm run check` for formatting and linting.

Run `npm run build` to check TypeScript types and build the frontend into `dist`.

Run `npm run tauri build` to build the desktop app. Rust and the macOS Xcode Command Line Tools are required.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
