# Ticket T03 — Host Contract Extensions (Sessions, Settings Draft, Reduced Transparency)

**Phase:** Phase 1 — Foundation
**Status:** Ready for Implementation
**Traceability:** R1, R3, R7, D2, Spec §4.1, §4.2
**Depends On:** None (Model only; parallel with T00–T02)
**Depended Upon By:** T10, T11, T12, T14

---

## 1. Description & Scope

Extend host state and action contracts in `crates/volumectl/src/ui/model.rs`, `capabilities.rs`, and wire execution arms in `crates/volumectl/src/host_core.rs`:
1. Add audio session state collection to `AppState`.
2. Add session volume and mute intents to `AppAction`.
3. Wire Settings draft and commit commands (`ApplyConfig`, `CancelConfig`, `ResetConfig`) in `host_core`.
4. Add `reduced_transparency` to `UiCapabilities` and update `resolve_material` to force opaque fallback.
5. Add `accent` (`AccentMode`) and `language` (`Language`) fields to `AppState` and action variants.

---

## 2. Planned Changes

1. **`crates/volumectl/src/ui/model.rs`**:
   ```rust
   #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
   pub struct AudioSessionState {
       pub id: String,
       pub name: String,
       pub process_path: Option<String>,
       pub volume_percent: u8,
       pub muted: bool,
   }

   pub struct AppState {
       // ... existing fields ...
       pub sessions: Vec<AudioSessionState>,
       pub accent: AccentMode,
   }

   pub enum AppAction {
       // ... existing variants ...
       SetSessionVolume { id: String, percent: u8 },
       ToggleSessionMute { id: String },
       SetAccent(AccentMode),
       SetLanguage(Language),
   }
   ```
2. **`crates/volumectl/src/ui/capabilities.rs`**:
   - Add `pub reduced_transparency: bool` to `UiCapabilities`.
   - Update `resolve_material`:
     ```rust
     if caps.high_contrast || caps.reduced_transparency || requested == MaterialMode::Opaque {
         return ResolvedMaterial::Opaque;
     }
     ```
3. **`crates/volumectl/src/host_core.rs`**:
   - Implement `AppAction::SetSessionVolume` calling `self.sessions_source.set_volume(id, pct)`.
   - Implement `AppAction::ToggleSessionMute` calling `self.sessions_source.mute(id)`.
   - Wire `AppAction::ApplyConfig`, `CancelConfig`, `ResetConfig` to persist and reload `config.ini`.

---

## 3. Acceptance Criteria

- [ ] `AppState` with `sessions` and `accent` serializes and deserializes cleanly via serde.
- [ ] `resolve_material` returns `ResolvedMaterial::Opaque` when `reduced_transparency == true`.
- [ ] Host routes session volume and mute actions to underlying audio session provider.
- [ ] `ApplyConfig` saves `config.ini`; `CancelConfig` reverts draft without touching disk.

---

## 4. Verification

```bash
cargo test -p volumectl --lib ui::model::tests
cargo test -p volumectl --lib ui::capabilities::tests
cargo test -p volumectl --lib host_core::tests
```
