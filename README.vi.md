# VolumeControl

Ứng dụng điều khiển âm lượng **native**, nhẹ — với phím tắt toàn cục, biểu
tượng khay hệ thống và overlay hiển thị mức âm — viết bằng Rust.

Kế thừa tinh thần của [VolumePro](https://github.com/dt418/VolumeControl)
(AutoHotkey): cùng mô hình tương tác, nhưng xây dựng lại thành ứng dụng
native đa nền tảng — không webview, không Electron, không phụ thuộc runtime
ngoài hệ điều hành.

## Tính năng

- **Phím tắt toàn cục** (mặc định `Ctrl+Alt`):
  - `Ctrl+Alt+↑ / ↓` — tăng/giảm âm lượng ±2%
  - `Ctrl+Alt+Shift+↑ / ↓` — tăng/giảm ±10%
  - `Ctrl+Alt+M` — bật/tắt tiếng
  - `Ctrl+Alt+R` — đặt lại 50%
  - `Ctrl+Alt+V` — mở mixer *(đang lên kế hoạch)*
  - `Ctrl+Alt+Shift+M` — mở menu khay (hoạt động cả khi Windows ẩn biểu
    tượng trong phần icon ẩn)
- **Phím media** (`Volume Up/Down/Mute`) giữ flyout gốc của Windows — ứng
  dụng chỉ đồng bộ trạng thái.
- **Overlay**: popup góc dưới-phải, thanh màu theo ngưỡng (xám / xanh lá /
  xanh dương / cam-đỏ) kèm phần trăm; tự ẩn sau ~1,8 giây; không bắt chuột.
- **Khay hệ thống**: nhãn âm lượng trực tiếp, bật/tắt tiếng, đặt lại 50%,
  thoát.
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
  "volume_step": 2,           // bước nhỏ, phần trăm (1-50)
  "volume_step_large": 10,    // bước Shift, phải > volume_step
  "overlay_duration_ms": 1800, // thời gian hiển thị overlay (200-10000)
  "modifier": "CtrlAlt",      // CtrlAlt | CapsLock | Alt | Ctrl
  "blacklist": [],            // dành cho phiên bản sau
  "color_thresholds": { "green_up_to": 40, "blue_up_to": 75, "orange_up_to": 100 }
}
```

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

- **macOS / Linux**: chưa hoàn thiện — hiện binary chạy dạng CLI đơn giản
  (`volumectl get` / `set <0-100>`). Kiến trúc (trait `AudioBackend` / hotkey)
  đã sẵn sàng cho backend native: CoreAudio trên macOS, PulseAudio/PipeWire
  trên Linux. Hợp đồng UI thích ứng dùng chung và các seam renderer biên dịch
  an toàn đã có, nhưng renderer AppKit và GTK/libadwaita là công việc ở giai
  đoạn sau — **chưa** được triển khai và chưa có gì trên macOS/Linux được kiểm
  chứng trong kế hoạch này.

## Trạng thái nền tảng

| Tính năng        | Windows | macOS | Linux |
|------------------|:-------:|:-----:|:-----:|
| Điều khiển âm lượng | ✅ WASAPI | 🔜 CoreAudio | 🔜 PulseAudio/PipeWire |
| Phím tắt toàn cục | ✅ RegisterHotKey | 🔜 | 🔜 |
| Overlay          | ✅ | 🔜 | 🔜 |
| Mixer            | ✅ | 🔜 | 🔜 |
| Cửa sổ Settings  | ✅ | 🔜 | 🔜 |
| Khay hệ thống    | ✅ tray-icon | 🔜 | 🔜 |
| Cấu hình trực tiếp | ✅ | — | — |
| Renderer UI thích ứng | ✅ native Win32 | 🔜 AppKit (chỉ seam) | 🔜 GTK/libadwaita (chỉ seam) |

## Kiến trúc

```
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
