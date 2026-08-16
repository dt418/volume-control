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

  Trước khi chạy WDIO, phải đặt rõ `E2E_DRIVER_PROVIDER=embedded` (hoặc
  `tauri-driver` sau khi preflight). Kiểm tra provider bằng
  `node e2e/tauri/test-provider.mjs --provider embedded --platform windows`.
  Các shortcut card của WDIO chỉ chứng minh UI/cấu hình; Linux/Xvfb và macOS
  hosted không chứng minh việc giao shortcut native. Trên Windows thật, chạy:

  `pwsh -NoProfile -File scripts/verify-hotkey-latency.ps1 -Release -Iterations 10 -OutputRoot output/manual/hotkey-latency`

  Probe gửi shortcut `open_mixer` đã cấu hình bằng `keybd_event`, đo trên OS
  thật và ghi báo cáo vào `output/manual/hotkey-latency/hotkey-latency.json`
  cùng `.txt`; không chạy probe trên Linux/macOS và không dùng báo cáo này làm
  bằng chứng Pilot tổng hợp.

  Xem [checklist bằng chứng phát hành đa nền tảng](docs/testing/cross-platform-release-checklist.md)
  để biết đầy đủ các bước kiểm tra, mức độ tin cậy và giới hạn của từng nền tảng.

## Trạng thái nền tảng

| Tính năng        | Windows | macOS | Linux |
|------------------|:-------:|:-----:|:-----:|
| Điều khiển âm lượng | ✅ WASAPI | ⚠️ CoreAudio host — manual | ⚠️ PulseAudio/PipeWire host — manual |
| Phím tắt toàn cục | ✅ RegisterHotKey | ⚠️ global-hotkey host — manual | ⚠️ global-hotkey host — manual |
| Overlay          | ✅ native Win32 | ⚠️ host — manual | ⚠️ host — manual |
| Mixer            | ✅ Tauri + session WASAPI | ✅ Tauri / ✖ audio từng app (không có public API) | ✅ Tauri + PulseAudio sink-inputs |
| Cửa sổ Settings  | ✅ Tauri | ✅ Tauri | ✅ Tauri |
| Khay hệ thống    | ✅ tray-icon | ✅ Tauri tray (menu-bar — manual) | ✅ Tauri tray (appindicator — manual) |
| Cấu hình trực tiếp | ✅ | — | — |
| Renderer UI thích ứng | ✅ native Win32 | ✅ AppKit (surface + smoke test) | ✅ GTK4/libadwaita (surface, CI test dưới Xvfb) |

`⚠️` nghĩa là host/backend hoặc surface đã có nhưng cần bằng chứng thủ công
trên desktop thật; `🔜` chỉ phần chưa được triển khai hoặc chưa có trong host.

macOS và Linux chạy host `global-hotkey` cùng backend audio native. Các surface
Tauri Settings/Help/Mixer dùng chung; enumeration session từng app và overlay/
tray native vẫn ưu tiên Windows. Renderer macOS/Linux triển khai cùng hợp đồng
Signal Glass thông qua bridge `NativeRenderer` dùng chung.

Bảng trên mô tả các surface đã triển khai, không có nghĩa mọi hosted runner đều
chứng minh được tích hợp native của hệ điều hành. WDIO chỉ chứng minh surface,
UI/cấu hình và IPC. Việc giao shortcut native, tray, audio phần cứng,
TCC/Accessibility, menu-bar, compositor Wayland và bố cục nhiều màn hình cần
kiểm tra thủ công theo
[checklist bằng chứng phát hành đa nền tảng](docs/testing/cross-platform-release-checklist.md).

## CI và bản phát hành

GitHub Actions (`.github/workflows/`) chạy các kiểm tra xác định và validation
desktop theo loại event:

- **Windows** — build, toàn bộ test suite và WDIO release gate trên Windows.
- **macOS** — build và smoke test renderer AppKit trong validation đầy đủ khi
  merge/phát hành.
- **Ubuntu 24.04** — build/test CLI fallback, smoke test GTK4/libadwaita dưới
  Xvfb và compile layer-shell tùy chọn trong validation đầy đủ.
- **Desktop E2E** — WebdriverIO/Tauri kiểm tra Mixer, Settings, Help, recovery,
  owned windows và runtime bridge; Tauri Pilot chỉ dùng cho replay/debug. Các
  job headless không chứng minh hotkey native, tray, audio phần cứng,
  compositor Wayland hay nhiều màn hình.

Push tag `v*` trước hết kiểm tra format của tag, sau đó chạy matrix desktop
Windows/macOS/Ubuntu có ràng buộc SHA. Job publish kiểm tra metadata, checksum
package và nội dung package trước khi đưa archive phiên bản cùng
`SHA256SUMS.txt` lên release; job này không build lại binary chưa được validate.
JUnit, manifest và log nền tảng của E2E được tạo và review riêng trong
validation artifact; verifier của publish không kiểm tra lại các file đó.

Package macOS hiện được ký ad-hoc để validation và kiểm tra local, không phải
chữ ký phân phối công khai. Phân phối macOS trong tương lai cần workflow được
bảo vệ với Developer ID và notarization (`notarytool`, stapling, `spctl` và
cleanup keychain tạm thời); secret ký không được lưu trong repository. Xem
[checklist bằng chứng phát hành đa nền tảng](docs/testing/cross-platform-release-checklist.md)
để biết lệnh kiểm tra và ranh giới signing chính xác.

Có thể phát hành từ GitHub UI: mở **Actions → Release → Run workflow**, chọn
branch nguồn đang trỏ đúng vào tag phiên bản đã tồn tại, nhập tag (ví dụ
`v0.1.0`), rồi chạy workflow. Preflight sẽ resolve cả annotated tag và từ chối
nếu tag không trỏ tới đúng `github.sha` được chọn; push tag là đường phát hành
được khuyến nghị.

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
