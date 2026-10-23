# 0001: Port Rust/UI Design Patterns to React 19 + Tailwind v4

- **Status**: Accepted
- **Date**: 2026-10-23
- **Context**: VolumeControl requires a modernized visual language inspired by Rust/UI (`rust-ui.com`) and Apple's Liquid Glass visual system. The current webview implementation uses React 19, Tailwind CSS v4, and Radix UI.
- **Decision**: Port Rust/UI component structures, Tailwind v4 utility tokens, and compact variants (`CardSm`, capsule pills, tactile sliders) directly into the existing React 19 component library rather than replacing webviews with Leptos WASM or Dioxus.
- **Consequences**:
  - Maintains the existing lightweight bundle (518 kB uncompressed, 157 kB gzip) with zero extra runtime WASM overhead.
  - Preserves all 43 passing frontend test suites (`vitest`) and existing Tauri v2 IPC bindings.
  - Avoids double-compilation of Rust for both native backend and WebAssembly frontend.
