# App i18n (all-in) + tray update-check menu

Scope (user: all-in): EN + VI everywhere except the NSIS installer shell
(tauri 2.11 bundle exposes no language MUI key in this repo's tauri.conf;
documented limitation).

## Source of truth

`config.ini [general] language = en | vi`, default `en`, unknown values
normalize to `en`. Owned by Rust `Config`; frontend and tray read it.

## Phase 1 — Rust locale + bilingual tray (foundation)

- `crates/volumectl/src/config.rs`: add `language: Language` (`En`/`Vi`,
  serde default `en`, normalize on load like other fields). Extend
  `host_core.rs` matrix with language default/normalize cases.
- New `crates/volumectl/src/locale.rs`: `pub enum Lang`, `t(key) -> &'static str`
  tables keyed by stable IDs (tray labels, overlay toasts, mixer/settings/gtk
  button text). Unit tests: every key non-empty in both languages; unknown
  language input can never occur (type-level).
- `tray.rs`: build menu labels via `locale` from the loaded config; menu IDs
  (`mute`, `reset`, …) stay untranslated. GTK (`platform/linux/renderer.rs`)
  and Win32 button labels (`mixer.rs`, `settings.rs` status text, `app.rs`
  toasts) switch to `locale` calls at their existing call sites.
- Language change path: `set_language` Tauri command (mirrors `set_modifier`
  in `src-tauri/src/commands.rs` + both `generate_handler!` lists in
  `lib.rs`), then trigger the same tray refresh the `ReloadConfig` path uses.

## Phase 2 — Tray "Check for updates"

- New `TrayCommand::CheckUpdates`, menu id `check_updates`, labels from
  `locale` (EN "Check for updates", VI "Kiểm tra cập nhật").
- `AppCore::check_for_updates()`: ureq (new workspace dep, rustls-tls,
  10s timeout) GET `api.github.com/dt418/volume-control/releases/latest`
  with `Accept: application/vnd.github+json` + `User-Agent`; pure
  `is_newer(latest, current)` semver compare against `CARGO_PKG_VERSION`;
  overlay toast result in the active language (up-to-date / version
  available / check failed; never throws to the tray loop).
- Tests: `is_newer` matrix (major/minor/patch, equal, malformed tags),
  `from_menu_id("check_updates")` decode extension.

## Phase 3 — Frontend i18n infra

- `frontend/src/lib/i18n.ts`: `en`/`vi` dictionaries (typed keys),
  `LangProvider` + `useT()`, language from `get_bootstrap` config
  (`SettingsConfig.language` + `hydrateConfig` default `en`).
- Settings General section: language switcher calling `set_language`.
- Vitest: dictionary parity (same key sets), provider renders both locales.

## Phase 4 — Per-surface string extraction (parallel slices)

Slices: mixer / settings / help+overlay+components. Each: replace literals
with `t()` + dictionary entries; surface tests render under fixed EN locale
plus one VI assertion that key strings flip. No behavior changes.

## Phase 5 — PowerShell bootstrap bilingual

- `Install-VolumeControl.ps1`: message table EN/VI, language from
  `$PSUICulture` (`vi*` → VI else EN) with `$env:VOLUMECONTROL_LANG`
  override (tests use the override); all user-facing throw/write strings go
  through the table. Version/install-root logic untouched.
- `test-install-volumecontrol.ps1`: run the suite under both overrides;
  message assertions become per-language.

## Phase 6 — Gate + commits

Per slice: `cargo fmt --check`, `git diff --check`, `cargo clippy
--workspace --all-targets --no-default-features -- -D warnings`,
`cargo test --workspace --no-default-features`, frontend tests,
PS suite, records (`feature_list.json` + `claude-progress.md` +
`session-handoff.md`) in the same commit. Slices: (1) rust locale+tray,
(2) update-check, (3) frontend infra, (4-6) per-surface, (7) PS bootstrap.

## Out of scope

NSIS installer-shell language; macOS/Linux native dialogs; new languages
beyond EN/VI (tables are additive when needed); REA audit (separate todo,
runs on the built app after these phases merge).
