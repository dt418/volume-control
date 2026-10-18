# VolumeControl v0.1.8 Release and Landing Page i18n Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate full bilingual (English / Vietnamese) support into the GitHub Pages landing site (`docs/index.html`), bump the project version to `v0.1.8` across all package manifests and changelog, and execute the release workflow.

**Architecture:**
- **Landing Page i18n (`docs/index.html`):** Add a seamless language switcher (EN / VI) in the navigation header, backed by complete bilingual content dictionaries covering the hero section, simulated overlay demo, remote installation cards, features grid, platform support table, trust boundaries, and footer. Language preference is persisted in `localStorage` and respects browser culture.
- **Release 0.1.8 Bump:** Synchronize version `0.1.8` across workspace `Cargo.toml`, `crates/volumectl/Cargo.toml`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `frontend/package.json`, and `frontend/package-lock.json`. Update `CHANGELOG.md` with release notes.
- **Release Publication:** Complete quality gates, merge PR to `main`, and push annotated git tag `v0.1.8` to trigger the automated multi-platform desktop release workflow (`.github/workflows/release.yml`).

**Tech Stack:** HTML5, CSS3, JavaScript, Rust 1.82+, Tauri v2, GitHub Actions.

## Global Constraints

- GitHub Pages (`docs/index.html`) must remain self-contained (vanilla HTML/CSS/JS, zero external CDN frameworks).
- Strict security constraints: never pipe downloads directly into a shell on Windows (`no | iex`). All release contract checks in `scripts/test-release-workflow.sh` must pass.
- Version string format must strictly adhere to `0.1.8` and git tag `v0.1.8`.
- Quality gates must pass with `-D warnings`.

---

### Task 1: GitHub Pages Landing Site Bilingual (EN/VI) Integration

**Files:**
- Modify: `docs/index.html`
- Test: `scripts/test-release-workflow.sh`

- [ ] **Step 1: Inspect and map all translatable content in `docs/index.html`**
- [ ] **Step 2: Add language switcher button in header navigation**
- [ ] **Step 3: Implement client-side i18n dictionary and data-i18n attribute rendering**
- [ ] **Step 4: Verify all release workflow checks pass (`bash scripts/test-release-workflow.sh`)**

---

### Task 2: Version Bump to 0.1.8 and Changelog Update

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/volumectl/Cargo.toml`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `frontend/package.json`
- Modify: `frontend/package-lock.json`
- Modify: `Cargo.lock`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Bump version numbers to 0.1.8 across all manifests**
- [ ] **Step 2: Refresh Cargo.lock and verify build**
- [ ] **Step 3: Document v0.1.8 changes in CHANGELOG.md**

---

### Task 3: Quality Gates, Records Registration, PR & Tag v0.1.8

**Files:**
- Modify: `feature_list.json`
- Modify: `claude-progress.md`

- [ ] **Step 1: Update feature_list.json and claude-progress.md**
- [ ] **Step 2: Run full quality gates (fmt, clippy, cargo test, npm test, deadlock, records)**
- [ ] **Step 3: Create PR on GitHub, merge to main, tag v0.1.8 and push**
