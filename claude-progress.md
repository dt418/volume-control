# Progress Log

## Current Verified State

- Repository root: D:\Projects\volume-control
- Standard startup path: `scripts/win-build.bat run` or `cargo run` (workspace default member = crates/volumectl) — MUST run through vcvars (MSVC env)
- Standard verification path: `scripts/win-build.bat build` then `scripts/win-build.bat test`
- Current highest-priority unfinished feature: vol-007 (macOS/Linux scaffolds + docs)
- Current blocker: none
- PASSING with recorded evidence: vol-001..vol-006 (workspace, audio, hotkeys, overlay, tray, config live reload + sync)

## Session Log

### Session 001 (2026-08-03)

- Date: 2026-08-03
- Goal: Scaffold the Cargo workspace for the VolumeControl Rust app; set up the project harness (skills, plugins, templates).
- Completed:
  - Cargo workspace scaffolded (root Cargo.toml + crates/volumectl with lib + bin).
  - Core modules drafted: config.rs, core.rs, audio/mod.rs (trait), audio_windows.rs (WASAPI), hotkeys/mod.rs, hotkeys_win32.rs (RegisterHotKey), cli.rs, main.rs, app.rs (Win32 message loop shell).
  - Harness setup COMPLETE:
    - superpowers plugin 6.2.0 installed (project scope, enabled) — 14 skills (brainstorming, writing-plans, executing-plans, test-driven-development, subagent-driven-development, systematic-debugging, verification-before-completion, etc.) + SessionStart hook. Skills on disk at ~/.claude/plugins/cache/superpowers-marketplace/superpowers/6.2.0/skills/. NOTE: SessionStart hook activates only on a fresh Claude Code session.
    - rtk 0.44.2 installed (prebuilt x86_64-pc-windows-msvc binary at ~/.cargo/bin/rtk.exe — cargo install from git failed on icu_normalizer_data build script). Project scope: CLAUDE.md instructions + .rtk/filters.toml. Global PreToolUse hook registered in ~/.claude/settings.json (backup at settings.json.bak), verified auto-rewriting `git status` → `rtk git status`. Uninstall: `rtk init -g --uninstall` + remove CLAUDE.md block.
    - learn-harness-engineering templates applied: CLAUDE.md (harness version), feature_list.json (8 features vol-001..vol-008), claude-progress.md, init.sh (cargo-adapted).
    - caveman NOT installed: research showed it is a caveman-speak communication-style skill (not a technical harness), ~1-1.5k input tokens/turn overhead vs modest output savings (net-negative on terse workloads). Reversible toggle later via `claude plugin marketplace add JuliusBrussee/caveman && claude plugin install caveman@caveman` if desired.
  - Git repo initialized at D:\Projects\volume-control (needed by superpowers worktrees).
- Verification run: `cargo build` green (0 errors, 0 warnings) via scripts/win-build.bat (vcvars64 wrapper); `cargo test` 0 failed.
- Evidence captured: end-to-end hotkey test — AHK SendInput Ctrl+Alt+Up/Down → WM_HOTKEY (ids 1/2) → apply() → WASAPI set_volume; system volume 98% → 100% confirmed by get_state.
- Commits: initial checkpoint commit pending (working tree has the full scaffold + verified core).
- Files or artifacts updated: see "Completed" above; also scripts/win-build.bat, target/debug/volumectl.exe.
- Known risk or unresolved issue:
  - MSVC toolchain setup: this machine had NO C linker — installed MSVC Build Tools 17.14 + Windows SDK 10.0.26100. Builds MUST run through scripts/win-build.bat (sets PATH/LIB/INCLUDE via vcvars64.bat).
  - Ctrl+Alt+M/R/V conflict with the running VolumePro AHK script (same default modifier) — handled gracefully (logged + skipped); user can change modifier in config.json.
  - overlay.rs COMPLETE + verified (vol-004 passing): GDI-painted Win32 popup, bottom-right, threshold colors, click-through (WS_EX_LAYERED|TRANSPARENT), auto-hide timer. Verified via EnumWindows visibility transitions + screenshot.
  - tray.rs COMPLETE + verified (vol-005 passing): tray-icon + muda menu (Volume % live label, Mute check, Reset 50%, separator, Exit). Tray icon found via UIA; menu captured in screenshot (Reset to 50% / Exit items); clean exit verified via WM_QUIT (same path as menu Exit). Added Ctrl+Alt+Shift+M OpenMenu hotkey (reachable even when icon is in the overflow flyout). NOTE: automated menu clicking is flaky on Windows 11 tray virtualization — items confirmed visually instead.
  - Config live reload COMPLETE + verified (vol-006 passing): mtime watch in the 150ms timer; volume_step 2->10 mid-run produced 10%/press deltas (88->86->76); modifier change re-registers hotkeys; load() save-if-changed avoids reload loops.
  - superpowers plugin SessionStart hook + rtk PreToolUse hook activate on a fresh Claude Code session.
- Next best step: vol-007 — README.md (EN + VI), macOS/Linux compile-gated scaffolds status (cli.rs fallback exists; audio/hotkeys mac/linux stubs pending), then vol-008 final end-to-end verify + release build.
