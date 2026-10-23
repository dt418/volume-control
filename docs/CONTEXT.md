# VolumeControl UI Domain Model

Domain language and boundary definitions for VolumeControl's visual architecture, material treatments, and multi-surface rendering system.

## Material & Glass System

**Surface Shell**:
The top-level window frame or HUD chrome container that directly interfaces with the operating system desktop compositor.
_Avoid_: Window wrapper, outer box, frame layer

**Content Layer**:
The interior visual layer containing typography, interactive controls, inputs, and session cards housed within a Surface Shell.
_Avoid_: Foreground, inner view, card content

**Liquid Glass**:
A multi-stage optical material system incorporating background refraction blur, luminance-adjusted tinting, and a specular rim highlight.
_Avoid_: Frosted glass, glassmorphism, blur filter, translucent sheet

**Specular Rim**:
A directional 1px inner highlight gradient along top and leading edges that simulates physical light refraction through glass.
_Avoid_: Top border, white stroke, bevel, edge glow

**Luminance Tint**:
A color adjustment layer operating in perceptual color space (OKLab) to modulate background brightness without clipping RGB channels.
_Avoid_: Color overlay, opacity layer, flat tint

**Anti-Blur Multiplication**:
The strict architectural constraint prohibiting nested `backdrop-filter: blur()` containers to prevent compositor degradation and readability loss.
_Avoid_: Double blur, layered blur, stacked filtering

## Configuration & Modes

**Material Mode**:
The user-selected surface translucency policy (`Auto`, `Translucent`, or `Opaque`) stored in `config.ini`.
_Avoid_: Glass mode, transparency setting, window style

**Motion Mode**:
The animation execution policy (`Full`, `Reduced`, or `Disabled`) dictating spring dynamics and transition durations across all surfaces.
_Avoid_: Animation speed, motion setting, transition effect

**Accent Mode**:
The active color palette selection (`System`, `Blue`, `Green`, `Purple`, or `Orange`) applied to specular highlights and Signal Rail indicators.
_Avoid_: Theme color, highlight color, tint selection

**Signal Rail**:
The hardware-calibrated, color-banded volume slider displaying current audio amplitude thresholds (gray, green, blue, orange).
_Avoid_: Volume bar, audio slider, progress rail

## Bilingual Nomenclature (EN / VI)

| Canonical English Term | Canonical Vietnamese Term | Contextual Definition |
|---|---|---|
| Surface Shell | Khung bề mặt | Container cửa sổ chính tiếp xúc với compositor của hệ điều hành. |
| Content Layer | Lớp nội dung | Vùng hiển thị chữ, thẻ và nút bấm tương tác bên trong bề mặt. |
| Liquid Glass | Kính chất lỏng (Liquid Glass) | Hệ thống vật liệu quang học đa tầng (blur, tint độ sáng, viền phản xạ khúc xạ). |
| Specular Rim | Viền phản xạ khúc xạ | Viền 1px mô phỏng ánh sáng môi trường chiếu qua mép kính. |
| Luminance Tint | Phủ màu độ sáng | Lớp điều chỉnh độ sáng trong không gian màu OKLab. |
| Material Mode | Chế độ vật liệu | Thiết lập độ trong suốt bề mặt: Tự động, Trong mờ, Đục. |
| Motion Mode | Chế độ chuyển động | Thiết lập hiệu ứng động: Đầy đủ, Thu gọn, Tắt. |
| Accent Mode | Màu chủ đạo | Bảng màu điểm nhấn: Hệ thống, Xanh dương, Xanh lá, Tím, Cam. |
| Signal Rail | Thanh tín hiệu âm lượng | Thanh trượt âm lượng phân dải màu theo ngưỡng phần cứng. |
