# Webview Compatibility (cross-platform)

**Applies to:** the Tauri v2 webview surfaces (Mixer, Settings, Help).
**Stack:** React 19, Tailwind CSS v4, shadcn-style components, framer-motion.
**Last verified:** 2026-08-13, live primary sources (research in
`.pi/subagents/artifacts/outputs/8b927a8f/research.md`).

Tauri does not bundle a webview — each platform uses the OS engine
([Tauri webview versions](https://v2.tauri.app/reference/webview-versions/)):

| Platform | Engine | Version floor | Tailwind v4 (Safari 16.4+) | backdrop-filter | Notes |
|---|---|---|---|---|---|
| Windows 10/11 | **WebView2** (Chromium, evergreen, self-updating) | none | ✅ | ✅ unprefixed (Chromium 76+) | Preinstalled on Win 11; runtime self-updates |
| macOS 13 Ventura | **WKWebView** (Safari 16.4 → 18.x) | ≥ 16.4 always | ✅ | ⚠️ prefix < 13.7; ✅ unprefixed 13.7+ | **Recommended min macOS floor** |
| macOS 12 Monterey | **WKWebView** (Safari 16.4 → **17.6** final) | any *patched* install | ✅ | ⚠️ prefix only (max 17.6) | Unpatched (Safari 15.x) is below floor |
| macOS 14 Sonoma | **WKWebView** (Safari 17.0 → 26.x) | ≥ 17.0 | ✅ | ⚠️ prefix < 14.7; ✅ unprefixed 14.7+ | Safest macOS target |
| Linux Ubuntu 22.04 amd64 (updated) | **WebKitGTK 4.1 2.50.4** (security pocket) | ≥ 2.40 (Tauri floor) | ✅ | ✅ unprefixed (2.46+) | Original release 2.36.0 is **below** the floor — must be `apt`-updated; **no arm64/ports builds** (stuck at 2.36.0) |
| Linux Ubuntu 24.04 amd64 | **WebKitGTK 4.1 2.44.0 → 2.52.3** | ≥ 2.44 original | ✅ | ✅ unprefixed (2.46+) | Primary Linux target |
| Linux Arch (rolling) | **WebKitGTK 4.1 2.52.5** | well above floor | ✅ | ✅ unprefixed | User's primary Linux distro |

## Engine floors (binding)

- **Tauri v2 build/runtime floor for Linux:** WebKitGTK 4.1 **≥ 2.40**
  (`tauri-runtime-wry` 2.11.4 → `wry` with `linux-body = ["webkit2gtk/v2_40"]`).
- **Tailwind v4 CSS floor:** Chrome 111 / **Safari 16.4** / Firefox 128
  (v4 relies on `@property` and `color-mix()`; v3.4 is the legacy fallback).
  Every supported target above clears it.
- **macOS floor policy:** macOS **13+ recommended** (12 Monterey patched is the
  minimum; Catalina/Big Sur are below the Tailwind floor).
- **framer-motion / React 19:** non-binding (practical floors far below).

## Fragile spots

1. **`backdrop-filter` prefix.** Safari/WebKit shipped the unprefixed property
   only from **Safari 18.0**; WebKit < 18 and WebKitGTK < 2.46 need the
   `-webkit-` prefix. Tailwind v4's `backdrop-blur-*` emits both declarations,
   but any hand-written blur must do the same.
2. **WebKitGTK software rendering.** With
   `WEBKIT_DISABLE_COMPOSITING_MODE=1` (or weak GPU/driver stacks) backdrop
   blur is expensive or silently no-ops. Every glass surface must therefore
   keep a **readable translucent background fallback** independent of the blur.

## Fallback pattern (used by the `.glass-surface` utility in `styles.css`)

```css
.glass-surface {
  background-color: color-mix(in oklab, var(--background) 85%, transparent);
}
@supports ((-webkit-backdrop-filter: blur(12px)) or (backdrop-filter: blur(12px))) {
  .glass-surface {
    -webkit-backdrop-filter: blur(12px); /* Safari 9–17, WebKitGTK ≤ 2.44 */
    backdrop-filter: blur(12px);         /* Safari 18+, WebKitGTK ≥ 2.46, Chromium 76+ */
    background-color: color-mix(in oklab, var(--background) 60%, transparent);
  }
}
```

The Mixer root uses `.glass-surface` (the window is transparent, so the root
needs an explicit readable background); Settings and Help roots use opaque
`bg-background`.

## Sources (primary, live-fetched)

- Tauri — [Webview versions](https://v2.tauri.app/reference/webview-versions/)
- Tauri — [Prerequisites](https://v2.tauri.app/start/prerequisites/)
- `wry` / `tauri-runtime-wry` Cargo.toml (webkit2gtk `v2_40` binding floor)
- Tailwind CSS — [Browser support](https://tailwindcss.com/docs/browser-support)
- WebKit — [WebKit Features in Safari 18.0](https://webkit.org/blog/15865/webkit-features-in-safari-18-0/)
  (unprefixed `backdrop-filter`), [CSS feature status](https://webkit.org/css-status/)
- packages.ubuntu.com (jammy/noble `libwebkit2gtk-4.1-0`)
- Arch — [webkit2gtk-4.1](https://archlinux.org/packages/extra/x86_64/webkit2gtk-4.1/)
- WebKitGTK — [releases](https://webkitgtk.org/releases/), bug
  [169988](https://www2.webkit.org/show_bug.cgi?id=169988) (GTK backdrop-filter),
  [EnvironmentVariables](https://trac.webkit.org/wiki/EnvironmentVariables)
- Motion — [FAQ](https://motion.dev/docs/faqs)
