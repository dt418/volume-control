# Ticket T16 — Release Cutover & Complete WebView Removal

**Phase:** Phase 4 — Cutover
**Status:** Blocked on T10–T15 Parity
**Traceability:** N1, N6, R2, Spec §1, §5
**Depends On:** T10, T11, T12, T13, T14, T15 (All surfaces verified at full native parity)
**Depended Upon By:** None (Final Release Gate)

---

## 1. Description & Scope

Execute the definitive, clean release cutover from hybrid webview UI to 100% native Rust/WGPU rendering. Delete all Tauri webview frontend assets, React code, node dependencies, transitional bridge shims, and IPC command handlers. Ensure zero dead code or compatibility shims survive into production.

---

## 2. Planned Changes

1. **Delete Frontend Directory**:
   - `rm -rf frontend/` (deletes React, Tailwind, Radix, Lucide, Vitest, TypeScript assets).
2. **Remove Webview Dependencies & Files**:
   - Remove `tauri`, `tauri-build`, and webview plugins from `src-tauri/Cargo.toml` and root `Cargo.toml`.
   - Delete `src-tauri/` webview bridge files and IPC command registration modules.
   - Delete transitional scaffolding and compatibility aliases.
3. **Clean App Entrypoint**:
   - `src/main.rs` binds directly to `volumectl::host_core` and `liquid_glass::LiquidGlassRenderer`.
4. **CI & Packaging Updates**:
   - Remove `npm install` and `npm run build` steps from GitHub Actions CI workflows.
   - Update NSIS and DMG packaging configurations to bundle only the self-contained native executable.

---

## 3. Acceptance Criteria

- [ ] Complete deletion of `frontend/` directory and all node/npm infrastructure.
- [ ] `cargo check --workspace --all-targets` succeeds with zero warnings.
- [ ] Release binary size decreases significantly (no embedded web assets or runtime engines).
- [ ] Cold startup time of VolumeControl drops to < 50 ms.
- [ ] Memory footprint under steady-state operation drops to < 30 MB.

---

## 4. Verification

```bash
cargo build --release --workspace
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```
