# Native Winit/WGPU Runtime Adapter Contract

## Status

Implemented in `crates/volumecontrol-host` (native Winit/WGPU host executable).
Retired 2026-10-25: the binary and crate were removed after the egui cutover;
this document is historical.
Windows visual and computer-use verification completed: all four surfaces and
update toast render with readable Vietnamese text; AccessKit exposes semantic
controls and computer-use toggles mute. WebView paths remain until native
keyboard/IME integration and macOS/Linux visual parity are proven.

## Ownership

`crates/liquid_glass` owns pure surface state, scene construction, GPU pipelines, and controller behavior. A host executable crate owns Winit `EventLoop`, native windows, `GpuContext`, `SurfaceManager`, and OS-specific placement. This prevents a `volumectl` ↔ `liquid_glass` dependency cycle.

```text
AppCore / host state publication
    -> LiquidGlassSurfaceController::publish(AppState, now, reduce_motion)
    -> per-surface Scene
    -> SceneFlattener
    -> content pass -> BlurPipeline::execute -> MaterialPipeline -> TextSystem
    -> WGPU SurfaceManager acquire/queue.present
```

## Runtime adapter responsibilities

1. Create one Winit event loop in the host executable.
2. Create windows for Overlay, Mixer, Settings, and Help according to authoritative `AppState.surfaces` visibility.
3. Use `GpuContext::init()` and `GpuContext::create_surface(window)` for every Winit window. Never pass a surface created by a foreign WGPU instance into context creation.
4. Manage each swapchain with `SurfaceManager` and the documented WGPU 30 recovery paths.
5. On every host publication, call `LiquidGlassSurfaceController::publish`.
6. On volume state changes, set Overlay visibility in the host publication before controller publish.
7. Send update check result strings through `LiquidGlassSurfaceController::show_notification`.
8. Route Winit pointer, keyboard, IME, and AccessKit events through input/a11y modules; translate user actions to `HostHandle::enqueue(AppAction)`.
9. Request redraw only after a host publication, input event, or active spring/toast animation. Stop redraw scheduling when `update()` reports idle.
10. Apply platform placement only in host adapter: Windows DX12 then Vulkan fallback, macOS Metal, Linux Vulkan and host-owned layer-shell overlay placement.

## Required proof before cutover

- Actual Winit window screenshots show native Overlay, Mixer, Settings, Help under WGPU.
- Volume change shows native HUD; `CheckUpdates` result shows native text toast.
- Host-state round trip verifies intent -> AppCore -> confirmed `AppState` -> controller scene update.
- WGPU surface smoke through selected backend on Windows, macOS, and Linux.
- WebView paths remain until all above proof exists.
