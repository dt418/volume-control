# Ticket T00 — Workspace & Crate Scaffold

**Phase:** Phase 1 — Foundation
**Status:** Ready for Implementation
**Traceability:** N1, N2, D1, Spec §5
**Depends On:** None
**Depended Upon By:** T01, T02, T04

---

## 1. Description & Scope

Establish the dedicated native renderer crate `crates/liquid_glass/` in the Cargo workspace and bump the workspace MSRV from 1.87 to 1.92 to satisfy the verified `glyphon 0.12` + `cosmic-text 0.19` dependency chain. Configure release and development profiles with Windows `static-dxc` compiler support.

---

## 2. Planned Changes

1. **Workspace Manifest (`Cargo.toml`)**:
   - Set `[workspace.package] rust-version = "1.92"`.
   - Add `"crates/liquid_glass"` to `workspace.members`.
   - Add pinned workspace dependencies:
     - `wgpu = "30.0.1"`
     - `glyphon = "0.12"`
     - `bytemuck = { version = "1.21", features = ["derive"] }`
     - `glam = "0.29"`
2. **New Crate Manifest (`crates/liquid_glass/Cargo.toml`)**:
   - Package name: `liquid_glass`, version `0.1.0`, edition `2021`, rust-version `1.92`.
   - Features: `default = []`, `static-dxc = ["wgpu/static-dxc"]`.
   - Dependencies: `wgpu`, `glyphon`, `bytemuck`, `glam`, `log`, `thiserror`.
   - Dev-dependencies: `winit = "0.30"`, `pollster = "0.4"`.
3. **Crate Root (`crates/liquid_glass/src/lib.rs`)**:
   - Declare core module stubs: `gpu`, `scene`, `material`, `blur`, `motion`, `text`, `input`, `a11y`.
   - Export top-level `LiquidGlassRenderer` and `Result` types.

---

## 3. Acceptance Criteria

- [ ] `cargo check --workspace --all-targets` succeeds on Rust 1.92+.
- [ ] `Cargo.lock` resolves `wgpu 30.0.1` and `glyphon 0.12`.
- [ ] Compiling `liquid_glass` with `--features static-dxc` succeeds on Windows without requiring external `dxcompiler.dll`.
- [ ] Zero foreign webview or generic widget crate dependencies (no egui, iced, dioxus).

---

## 4. Verification

```bash
cargo check -p liquid_glass --all-targets
cargo test -p liquid_glass
cargo check --workspace --all-targets
```
