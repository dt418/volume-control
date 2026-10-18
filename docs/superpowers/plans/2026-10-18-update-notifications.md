# Update Check and Notification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a comprehensive update check and notification system across the desktop application (Settings UI & tray), CLI (`volumectl check-updates`), and IPC event bridge (`state://update`), notifying users when a newer release is available and providing direct links and update instructions.

**Architecture:**
- **Core (`crates/volumectl/src/updates.rs`):** Provide structured `UpdateInfo` (current, latest, is_available, release_url, localized message) with pure semver comparison and network timeout guards.
- **CLI (`crates/volumectl/src/cli.rs`):** Expose `volumectl check-updates` command reporting localized update status to stdout.
- **Tauri IPC (`src-tauri/src/commands.rs` & `lib.rs`):** Expose `check_for_updates` command returning `UpdateInfo` and emitting `state://update` event to all open surface windows.
- **Frontend UI (`frontend/src/settings/`):** Add an Updates card in Settings (`GeneralSection`) displaying current version, a "Check for updates" button with loading/checked states, and a prominent notification banner when a newer release is detected.
- **Localization (`locale.rs` & `frontend/src/lib/i18n.ts`):** Bilingual support (EN and VI) for all update-related text and statuses.

**Tech Stack:** Rust (ureq, serde), Tauri v2 IPC, React 19, TypeScript, Vitest, Tailwind CSS.

## Global Constraints

- Network calls must have strict bounded timeouts (10s max) and fail soft (never panic or crash the host/tray).
- Version parsing must adhere strictly to `vMAJOR.MINOR.PATCH` semver comparison (`is_newer`).
- UI must be accessible (ARIA roles, keyboard navigable, high contrast compatible).
- Bilingual parity: every string must exist in both English (`en`) and Vietnamese (`vi`).
- Zero changes to volume mixing or hotkey latency paths.

---

### Task 1: Structured Update Check API and CLI Command

**Files:**
- Modify: `crates/volumectl/src/updates.rs`
- Modify: `crates/volumectl/src/cli.rs`
- Modify: `crates/volumectl/src/locale.rs`

**Interfaces:**
- Produces: `pub fn check_update_info(lang: Language) -> Result<UpdateInfo, String>`
- Produces: `volumectl check-updates` CLI subcommand.

- [ ] **Step 1: Write the failing unit tests**

In `crates/volumectl/src/updates.rs`:
```rust
#[test]
fn update_info_detects_newer_version() {
    let current = "0.1.7";
    let latest = "v0.1.8";
    assert!(is_newer(latest, current));
}
```

- [ ] **Step 2: Run test to verify it passes/fails**

Run: `cargo test -p volumectl --lib updates`

- [ ] **Step 3: Implement UpdateInfo struct and check_update_info**

In `crates/volumectl/src/updates.rs`:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub is_update_available: bool,
    pub release_url: Option<String>,
    pub message: String,
}

pub fn check_update_info(lang: Language) -> Result<UpdateInfo, String> {
    let s = locale::strings(lang);
    let current = env!("CARGO_PKG_VERSION").to_string();
    match fetch_latest_tag() {
        Ok(tag) => {
            let newer = is_newer(&tag, &current);
            let message = if newer {
                format!("{} {tag}", s.update_check_available_prefix)
            } else {
                s.update_check_up_to_date.to_string()
            };
            let release_url = if newer {
                Some(format!("https://github.com/dt418/volume-control/releases/tag/{tag}"))
            } else {
                None
            };
            Ok(UpdateInfo {
                current_version: current,
                latest_version: Some(tag),
                is_update_available: newer,
                release_url,
                message,
            })
        }
        Err(err) => Ok(UpdateInfo {
            current_version: current,
            latest_version: None,
            is_update_available: false,
            release_url: None,
            message: format!("{}: {err}", s.update_check_failed_prefix),
        }),
    }
}
```

In `crates/volumectl/src/cli.rs`:
Add `"check-updates"` subcommand:
```rust
        "check-updates" => {
            let lang = crate::config::load().language;
            let info = crate::updates::check_update_info(lang).map_err(|e| e.to_string())?;
            println!("{}", info.message);
            Ok(ExitCode::SUCCESS)
        }
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p volumectl --lib`

---

### Task 2: Tauri IPC Command & Update Notification Events

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `pub async fn check_for_updates(...) -> Result<UpdateInfo, String>`
- Emits: `state://update` event with `UpdateInfo` payload.

- [ ] **Step 1: Implement check_for_updates command**

In `src-tauri/src/commands.rs`:
```rust
#[tauri::command]
pub async fn check_for_updates(
    app: tauri::AppHandle,
    core: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> Result<volumectl_lib::updates::UpdateInfo, String> {
    let language = {
        let core = core.lock().map_err(|e| e.to_string())?;
        core.config().language
    };

    let info = tauri::async_runtime::spawn_blocking(move || {
        volumectl_lib::updates::check_update_info(language)
    })
    .await
    .map_err(|e| e.to_string())??;

    let _ = app.emit("state://update", &info);
    Ok(info)
}
```

- [ ] **Step 2: Register command in src-tauri/src/lib.rs**

Add `check_for_updates` to:
1. `use commands::{..., check_for_updates};`
2. `invoke_handler(tauri::generate_handler![..., check_for_updates])` (both lists in `lib.rs`).

- [ ] **Step 3: Run Tauri tests**

Run: `cargo test -p volumecontrol-tauri`

---

### Task 3: Frontend Updates Section & Notification Banner

**Files:**
- Modify: `frontend/src/settings/GeneralSection.tsx`
- Modify: `frontend/src/lib/i18n.ts`
- Test: `frontend/src/settings/GeneralSection.test.tsx`

**Interfaces:**
- Consumes: `check_for_updates` via `invoke`, `state://update` via `listen`.
- Produces: Updates card with version display, check button, loading state, and update alert banner.

- [ ] **Step 1: Write the failing frontend test**

In `frontend/src/settings/GeneralSection.test.tsx`:
Test that the Updates section renders current version, a "Check for updates" button, and handles update check responses.

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix frontend test -- GeneralSection`

- [ ] **Step 3: Implement Updates UI and bilingual translations**

In `frontend/src/lib/i18n.ts`:
Add keys:
- `settings.updates.title`: "Updates" / "Cập nhật"
- `settings.updates.current`: "Current version" / "Phiên bản hiện tại"
- `settings.updates.checkBtn`: "Check for updates" / "Kiểm tra cập nhật"
- `settings.updates.checking`: "Checking..." / "Đang kiểm tra..."
- `settings.updates.available`: "Update available" / "Có bản cập nhật mới"
- `settings.updates.upToDate`: "Up to date" / "Đã là bản mới nhất"

In `frontend/src/settings/GeneralSection.tsx`:
Add Update card displaying version, trigger button, and conditional alert banner when `updateInfo?.is_update_available` is true.

- [ ] **Step 4: Verify frontend tests pass**

Run: `npm --prefix frontend test -- GeneralSection`

---

### Task 4: Feature Registration, Pre-Push Review, and Gate

**Files:**
- Modify: `feature_list.json`
- Modify: `claude-progress.md`

- [ ] **Step 1: Register vol-102 in feature_list.json**
- [ ] **Step 2: Document Session 120 in claude-progress.md**
- [ ] **Step 3: Run full quality gate**

```bash
cargo fmt --all --check
git diff --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
npm --prefix frontend test -- --run
sh scripts/check-records.sh
sh scripts/check-tauri-deadlock.sh
```

- [ ] **Step 4: Commit**
