# VolumeControl

Ứng dụng điều khiển âm lượng **native-first**, nhẹ — với phím tắt toàn cục,
biểu tượng khay hệ thống, overlay hiển thị mức âm và các surface Mixer/
Settings/Help bằng Tauri v2 — viết bằng Rust và React/TypeScript.

Kế thừa tinh thần của [VolumePro](https://github.com/dt418/VolumeControl)
(AutoHotkey): cùng mô hình tương tác, nhưng xây dựng lại thành ứng dụng
native đa nền tảng. Host luôn chạy native; webview chỉ được tạo lười cho
Mixer, Settings và Help.

## Tính năng

- **Phím tắt toàn cục** (mặc định `Ctrl+Alt`, cấu hình riêng từng hành động
  trong Settings):
  - `Ctrl+Alt+↑ / ↓` — tăng/giảm âm lượng ±1%
  - `Ctrl+Alt+Shift+↑ / ↓` — tăng/giảm ±10%
  - `Ctrl+Alt+M` — bật/tắt tiếng
  - `Ctrl+Alt+R` — đặt lại 50%
  - `Ctrl+Alt+V` — bật/tắt mixer
  - `Ctrl+Alt+Shift+M` — mở menu khay (hoạt động cả khi Windows ẩn biểu
    tượng trong phần icon ẩn)
- **Đăng ký shortcut**: nhấn `Record` rồi gõ tổ hợp phím; `Clear` tắt riêng
  hành động đó, còn shortcut trùng/xung đột được báo trước khi lưu.
- **Mixer**: điều khiển output hệ thống, Signal Rail theo ngưỡng, session từng
  ứng dụng trên Windows, tìm kiếm và retry khi backend tạm thời lỗi.
- **Settings/Help**: chỉnh sửa theo draft rồi lưu nguyên tử; Help hiển thị
  shortcut và trạng thái đăng ký hiện tại.
- **Phím media** (`Volume Up/Down/Mute`) giữ flyout gốc của Windows — ứng
  dụng chỉ đồng bộ trạng thái.
- **Overlay**: popup góc dưới-phải, thanh màu theo ngưỡng (xám / xanh lá /
  xanh dương / cam-đỏ) kèm phần trăm; tự ẩn sau ~1,8 giây; không bắt chuột.
- **Khay hệ thống**: nhãn âm lượng trực tiếp, bật/tắt tiếng, đặt lại 50%,
  thoát.
- **Cấu hình ưu tiên qua Settings**: các tùy chọn chung, giao diện, blacklist,
  feedback, storage và shortcut có thể chỉnh trực tiếp trong ứng dụng. File
  JSON vẫn được giữ cho automation và chỉnh nâng cao.
- **Nạp lại cấu hình trực tiếp**: sửa `config.json` là áp dụng ngay trong
  ~150 ms — không cần khởi động lại.
- **Đồng bộ ngoài**: âm lượng đổi bởi phím media, ứng dụng khác hoặc
  Bluetooth được cập nhật tức thì trên khay.

## Cấu hình

Lần chạy đầu tiên, ứng dụng ghi cấu hình mặc định vào:

| Hệ điều hành | Đường dẫn |
|--------------|-----------|
| Windows | `%APPDATA%\volume-control\config.json` |
| macOS | `~/Library/Application Support/volume-control/config.json` |
| Linux | `~/.config/volume-control/config.json` |

```jsonc
{
  "volume_step": 1,           // bước nhỏ, phần trăm (1-50)
  "volume_step_large": 10,    // bước Shift, phải > volume_step
  "overlay_duration_ms": 1800, // thời gian hiển thị overlay (200-10000)
  "modifier": "CtrlAlt",      // CtrlAlt | CapsLock | Alt | Ctrl
  "blacklist": [],            // tên executable bị loại khỏi hotkey
  "color_thresholds": { "green_up_to": 40, "blue_up_to": 75, "orange_up_to": 100 },
  "hotkeys": {
    "volume_up": "Ctrl+Alt+ArrowUp",
    "volume_down": "Ctrl+Alt+ArrowDown",
    "volume_up_large": "Ctrl+Alt+Shift+ArrowUp",
    "volume_down_large": "Ctrl+Alt+Shift+ArrowDown",
    "toggle_mute": "Ctrl+Alt+KeyM",
    "reset_50": "Ctrl+Alt+KeyR",
    "open_mixer": "Ctrl+Alt+KeyV",
    "open_menu": "Ctrl+Alt+Shift+KeyM"
  }
}
```

Giá trị rỗng trong `hotkeys` sẽ tắt riêng hành động đó. Config cũ không có
object `hotkeys` sẽ tự chuyển sang preset của modifier đang chọn.

## Biên dịch

Yêu cầu: Rust (stable) + trình biên dịch C:

- **Windows**: MSVC Build Tools + Windows SDK. Biên dịch qua
  `scripts\win-build.bat` (gói `cargo` với môi trường MSVC của
  `vcvars64.bat`):

  ```bat
  scripts\win-build.bat build
  scripts\win-build.bat run
  scripts\win-build.bat test
  ```

- **macOS**: Rust (stable) + công cụ dòng lệnh Xcode:

  ```bash
  cargo build
  cargo test    # bao gồm smoke test renderer AppKit
  ```

- **Ubuntu 24.04** (hoặc Debian 12+): Rust (stable) + gói dev GTK4/libadwaita.
  Nếu không có chúng, binary chạy dạng CLI đơn giản (`volumectl get` /
  `set <0-100>`); nếu có, renderer native được biên dịch:

  ```bash
  sudo apt-get install libgtk-4-dev libadwaita-1-dev libpulse-dev xvfb
  cargo build                                    # CLI fallback
  cargo build --features gtk-renderer            # surface GTK4 native
  cargo build --features gtk-renderer,layer-shell  # + overlay/mixer layer-shell Wayland
  xvfb-run -a cargo test --features gtk-renderer # smoke test renderer
  ```

  Đường dẫn layer-shell Wayland cũng cần `libgtk-4-layer-shell-dev` (có trong
  Ubuntu 24.04); nếu thiếu, các surface dùng cửa sổ không viền tương thích X11.

- **Tauri webview**: Node.js 22+:

  ```bash
  npm ci --prefix frontend
  npm --prefix frontend run build
  npm --prefix frontend test
  ```

  Bộ E2E desktop nằm riêng trong `e2e/tauri`; cài bằng
  `npm ci --prefix e2e/tauri`. Trên Windows chạy
  `scripts\verify-tauri-e2e.ps1 -Surface all`. Hook dev/build của Tauri cũng
  dùng object command với `cwd: ../frontend`, nên lệnh frontend độc lập với
  thư mục gọi Tauri CLI và không còn tìm nhầm `package.json` ở thư mục sai.

## Trạng thái nền tảng

| Tính năng        | Windows | macOS | Linux |
|------------------|:-------:|:-----:|:-----:|
| Điều khiển âm lượng | ✅ WASAPI | 🔜 CoreAudio | 🔜 PulseAudio/PipeWire |
| Phím tắt toàn cục | ✅ RegisterHotKey | 🔜 | 🔜 |
| Overlay          | ✅ native Win32 | 🔜 host | 🔜 host |
| Mixer            | ✅ Tauri + session WASAPI | ✅ Tauri / 🔜 audio từng app | ✅ Tauri / 🔜 audio từng app |
| Cửa sổ Settings  | ✅ Tauri | ✅ Tauri | ✅ Tauri |
| Khay hệ thống    | ✅ tray-icon | 🔜 | 🔜 |
| Cấu hình trực tiếp | ✅ | — | — |
| Renderer UI thích ứng | ✅ native Win32 | ✅ AppKit (surface + smoke test) | ✅ GTK4/libadwaita (surface, CI test dưới Xvfb) |

macOS và Linux chạy host `global-hotkey` cùng backend audio native. Các surface
Tauri Settings/Help/Mixer dùng chung; enumeration session từng app và overlay/
tray native vẫn ưu tiên Windows. Renderer macOS/Linux triển khai cùng hợp đồng
Signal Glass thông qua bridge `NativeRenderer` dùng chung.

## CI và bản phát hành

GitHub Actions (`.github/workflows/`) kiểm tra mọi push/PR:

- **Windows** — build, toàn bộ test suite, kiểm tra artifact release.
- **macOS** — build và test gồm cả smoke test renderer AppKit.
- **Ubuntu 24.04** — build/test CLI fallback, build GTK4/libadwaita và smoke
  test renderer dưới Xvfb, cùng build layer-shell Wayland.
- **Desktop E2E** — WebdriverIO/Tauri kiểm tra Mixer, Settings, Help, recovery,
  owned windows và runtime bridge; Tauri Pilot chỉ dùng cho replay/debug.

Push tag `v*` sẽ cài/build frontend rồi chạy Tauri release build (embed asset
frontend + backend Rust) trên cả ba nền tảng trước khi xuất bản archive phiên
bản và `SHA256SUMS.txt` (`scripts/package.sh`).

## Kiến trúc

```
frontend/                    React + TypeScript + Vite webview surfaces
e2e/tauri/                   bộ WebdriverIO/Tauri E2E tách biệt
src-tauri/                   Tauri v2 host, command và window manager
crates/volumectl/
├── src/
│   ├── audio/          trait AudioBackend (đa nền tảng)
│   ├── audio_windows   WASAPI qua COM vtable thủ công (windows-sys)
│   ├── hotkeys/        các loại HotkeyAction
│   ├── hotkeys_win32   RegisterHotKey + vòng lặp message cửa sổ ẩn
│   ├── overlay         popup native vẽ bằng GDI (click-through, tự ẩn)
│   ├── tray            tray-icon + menu muda
│   ├── config          JSON, nạp lại trực tiếp theo mtime
│   ├── core            logic dùng chung (clamp, ngưỡng màu) + unit tests
│   ├── ui/             hợp đồng UI thích ứng dùng chung (model, theme,
│   │                   capabilities, surface, settings) + các seam renderer
│   └── cli             CLI fallback cho nền tảng khác
```

Các module chỉ dành cho Windows được gate bằng `#[cfg(target_os = "windows")]`;
crate vẫn biên dịch được trên macOS/Linux (dạng CLI), để thêm backend native
từng bước. Module `ui` định nghĩa hợp đồng renderer dùng chung;
`ui/platform/macos` và `ui/platform/linux` là các seam biên dịch an toàn
(hiện chỉ là stub) cho renderer AppKit và GTK/libadwaita ở giai đoạn sau.

## Giấy phép

MIT
