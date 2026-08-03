//! Shared Signal Rail state and geometry.
//!
//! The Signal Rail is VolumeControl's signature element: a threshold-aware
//! volume track with a precise thumb (normal) or diamond outline (muted)
//! marker, shared by the overlay and the mixer on every platform.
//!
//! Everything in this module is pure and platform-neutral: percentages and
//! logical floats only, no platform imports. Renderers consume the geometry
//! and choose their own paint API — the Windows canvas scales the logical
//! floats once via its DPI metrics, and the macOS/Linux renderers use the
//! same floats unchanged.
//!
//! # High contrast
//!
//! The muted state never relies on color alone. The muted marker is a
//! different *shape* — an outline diamond (`MutedDiamond`) rather than the
//! filled-circle thumb — so high-contrast and color-vision-limited users
//! distinguish it without tint. The geometry guarantees the two marker
//! variants differ; renderers pair the shape with the `Muted` label.
//!
//! # Palette authority
//!
//! [`SignalRail::fill_color`] mirrors the authoritative
//! [`crate::core::volume_color_rgb`] semantics (muted or 0% → muted grey,
//! then low/medium/high threshold bands) using the VolumePro band limits
//! from [`crate::config::ColorThresholds`]. The palette values themselves
//! come from the caller-provided [`VolumeThresholdColors`]; the rail never
//! carries its own color copy.

use crate::ui::theme::{Rgba, VolumeThresholdColors};

/// Low band upper bound in percent (inclusive), mirroring the VolumePro
/// default `green_up_to` in [`crate::config::ColorThresholds`]. Kept in one
/// place so [`SignalRail::fill_color`] cannot fork the authoritative band
/// semantics; the test suite asserts agreement with `core::volume_color_rgb`.
const LOW_BAND_UP_TO: u8 = 40;
/// Medium band upper bound in percent (inclusive), mirroring the VolumePro
/// default `blue_up_to` in [`crate::config::ColorThresholds`].
const MEDIUM_BAND_UP_TO: u8 = 75;

/// Platform-neutral Signal Rail state.
///
/// `percent` is in the range 0..=100. Callers are expected to keep it in
/// range; the rail also clamps defensively ([`Self::clamped`], step
/// arithmetic, and the geometry) so out-of-range values never escape into
/// geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalRail {
    pub percent: u8,
    pub muted: bool,
    pub thresholds: VolumeThresholdColors,
}

impl SignalRail {
    pub fn new(percent: u8, muted: bool, thresholds: VolumeThresholdColors) -> Self {
        Self {
            percent,
            muted,
            thresholds,
        }
    }

    /// Defensive copy with `percent` clamped to at most 100.
    pub fn clamped(&self) -> Self {
        Self {
            percent: self.percent.min(100),
            ..*self
        }
    }

    /// `percent + step`, clamped to at most 100. Mute and thresholds are
    /// preserved; stepping never changes the mute state.
    pub fn step_up(&self, step: u8) -> Self {
        Self {
            percent: self.clamped().percent.saturating_add(step).min(100),
            ..*self
        }
    }

    /// `percent - step`, clamped to at least 0. Mute and thresholds are
    /// preserved; stepping never changes the mute state.
    pub fn step_down(&self, step: u8) -> Self {
        Self {
            percent: self.clamped().percent.saturating_sub(step),
            ..*self
        }
    }

    /// 0%.
    pub fn home(&self) -> Self {
        Self {
            percent: 0,
            ..*self
        }
    }

    /// 100%.
    pub fn end(&self) -> Self {
        Self {
            percent: 100,
            ..*self
        }
    }

    /// Fill color for the current percent, mirroring the authoritative
    /// [`crate::core::volume_color_rgb`] semantics: muted (or 0%) uses the
    /// muted grey; otherwise the low/medium/high threshold band applies. The
    /// palette is the caller-provided `thresholds`, never a local copy.
    pub fn fill_color(&self) -> Rgba {
        if self.muted || self.percent == 0 {
            self.thresholds.muted
        } else if self.percent <= LOW_BAND_UP_TO {
            self.thresholds.low
        } else if self.percent <= MEDIUM_BAND_UP_TO {
            self.thresholds.medium
        } else {
            self.thresholds.high
        }
    }
}

/// Logical rectangle of the rail track. Values are logical floats; renderers
/// scale exactly once via their platform DPI metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackRect {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl TrackRect {
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn center_x(&self) -> f32 {
        (self.left + self.right) * 0.5
    }

    pub fn center_y(&self) -> f32 {
        (self.top + self.bottom) * 0.5
    }
}

/// Marker geometry. The enum variant is semantic: renderers must draw a
/// filled circle for [`Self::Thumb`] and an outline diamond (◇) for
/// [`Self::MutedDiamond`]. High-contrast and color-vision-limited users rely
/// on this shape difference plus the `Muted` label — never on tint alone —
/// so the muted marker is always a distinct shape, never a color-only
/// variant of the thumb.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarkerGeometry {
    /// Normal-state marker: a filled circle.
    Thumb {
        center_x: f32,
        center_y: f32,
        radius: f32,
    },
    /// Muted-state marker: an outline diamond (◇).
    MutedDiamond {
        center_x: f32,
        center_y: f32,
        half_size: f32,
    },
}

impl MarkerGeometry {
    /// Marker center as `(x, y)`, defined for every variant.
    pub fn center(&self) -> (f32, f32) {
        match *self {
            Self::Thumb {
                center_x, center_y, ..
            } => (center_x, center_y),
            Self::MutedDiamond {
                center_x, center_y, ..
            } => (center_x, center_y),
        }
    }
}

/// Resolved rail geometry for one frame: the track, the threshold-fill edge,
/// and the marker. All values are logical floats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignalRailGeometry {
    pub track: TrackRect,
    /// X where the threshold fill ends; the fill spans `track.left..=fill_right`.
    pub fill_right: f32,
    pub marker: MarkerGeometry,
}

/// Compute the rail geometry for `rail` inside `track`.
///
/// - `fill_right = track.left + track.width() * percent / 100`, with the
///   percent defensively clamped to 100 (0% → `track.left`, 100% →
///   `track.right`).
/// - Normal: a [`MarkerGeometry::Thumb`] centered at `fill_right`, clamped so
///   the thumb stays fully inside the track.
/// - Muted: a [`MarkerGeometry::MutedDiamond`] at the same clamped center,
///   sized by `muted_diamond_half_size`.
///
/// `thumb_radius` and `muted_diamond_half_size` are in logical px (spec
/// thumb: 6px radius, 12px diameter). If the track is narrower than the
/// marker's extent, the marker is centered in the track.
pub fn rail_geometry(
    rail: &SignalRail,
    track: TrackRect,
    thumb_radius: f32,
    muted_diamond_half_size: f32,
) -> SignalRailGeometry {
    let percent = f32::from(rail.percent.min(100));
    let fill_right = track.left + track.width() * (percent / 100.0);
    let center_y = track.center_y();
    let marker = if rail.muted {
        let half_size = muted_diamond_half_size.max(0.0);
        MarkerGeometry::MutedDiamond {
            center_x: clamp_center(fill_right, track.left + half_size, track.right - half_size),
            center_y,
            half_size,
        }
    } else {
        let radius = thumb_radius.max(0.0);
        MarkerGeometry::Thumb {
            center_x: clamp_center(fill_right, track.left + radius, track.right - radius),
            center_y,
            radius,
        }
    };
    SignalRailGeometry {
        track,
        fill_right,
        marker,
    }
}

/// Clamp `center` into `[min, max]`. A degenerate or inverted interval (track
/// narrower than the marker's extent) resolves to the interval midpoint so
/// the marker stays as close to the fill edge as the track allows.
fn clamp_center(center: f32, min: f32, max: f32) -> f32 {
    if max <= min {
        (min + max) * 0.5
    } else {
        center.clamp(min, max)
    }
}

/// Keyboard semantics for a Signal Rail. The mixer's native trackbar already
/// handles arrow keys natively; this maps the same semantics for any renderer
/// that needs them (custom chrome, non-native surfaces).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailKey {
    Home,
    End,
    ArrowUp,
    ArrowDown,
}

/// Apply a key to `rail`: Home → 0, End → 100, ArrowUp → [`SignalRail::step_up`],
/// ArrowDown → [`SignalRail::step_down`]. `step` is in percent; results clamp
/// to the 0..=100 range.
pub fn rail_apply_key(rail: &SignalRail, key: RailKey, step: u8) -> SignalRail {
    match key {
        RailKey::Home => rail.home(),
        RailKey::End => rail.end(),
        RailKey::ArrowUp => rail.step_up(step),
        RailKey::ArrowDown => rail.step_down(step),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ColorThresholds;
    use crate::core::volume_color_rgb;

    fn rail(percent: u8, muted: bool) -> SignalRail {
        SignalRail::new(percent, muted, VolumeThresholdColors::default())
    }

    /// Overlay-like track: 336px surface, 16px padding, 8px rail height.
    fn track() -> TrackRect {
        TrackRect {
            left: 16.0,
            right: 320.0,
            top: 64.0,
            bottom: 72.0,
        }
    }

    /// Spec thumb radius (12px diameter) with a same-sized diamond.
    fn geometry(rail: &SignalRail) -> SignalRailGeometry {
        rail_geometry(rail, track(), 6.0, 6.0)
    }

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() <= 1e-4
    }

    // --- fill edge ------------------------------------------------------------

    #[test]
    fn fill_right_matches_track_bounds_at_0_50_100() {
        let t = track();
        assert_eq!(geometry(&rail(0, false)).fill_right, t.left);
        assert_eq!(
            geometry(&rail(50, false)).fill_right,
            t.left + t.width() * 0.5
        );
        assert_eq!(geometry(&rail(100, false)).fill_right, t.right);
    }

    #[test]
    fn percent_beyond_100_defensively_clamps() {
        let t = track();
        for p in [101u8, 150, 255] {
            let g = geometry(&rail(p, false));
            assert_eq!(g.fill_right, t.right, "percent {p}");
            let MarkerGeometry::Thumb {
                center_x, radius, ..
            } = g.marker
            else {
                panic!("expected a thumb at percent {p}");
            };
            assert!(
                center_x >= t.left + radius - 1e-6 && center_x <= t.right - radius + 1e-6,
                "thumb at {center_x} outside [{}, {}]",
                t.left + radius,
                t.right - radius
            );
        }
        assert_eq!(rail(150, false).clamped().percent, 100);
        assert_eq!(rail(255, false).clamped().percent, 100);
        assert_eq!(rail(100, false).clamped().percent, 100);
        assert_eq!(rail(50, false).clamped().percent, 50);
    }

    #[test]
    fn thumb_stays_fully_inside_track_at_0_and_100() {
        let t = track();
        let radius = 6.0;
        let g0 = geometry(&rail(0, false));
        let g100 = geometry(&rail(100, false));
        let MarkerGeometry::Thumb {
            center_x: c0,
            radius: r0,
            ..
        } = g0.marker
        else {
            panic!("0% must be a thumb");
        };
        let MarkerGeometry::Thumb {
            center_x: c100,
            radius: r100,
            ..
        } = g100.marker
        else {
            panic!("100% must be a thumb");
        };
        assert_eq!(r0, radius);
        assert_eq!(r100, radius);
        // 0%: the fill sits at the left edge, so the thumb is pushed right.
        assert_eq!(c0, t.left + radius);
        // 100%: the fill sits at the right edge, so the thumb is pushed left.
        assert_eq!(c100, t.right - radius);
        for c in [c0, c100] {
            assert!(
                c - radius >= t.left && c + radius <= t.right,
                "thumb at {c} leaves the track"
            );
        }
    }

    #[test]
    fn marker_centers_when_track_is_narrower_than_the_marker() {
        // 8px track cannot fit a 12px thumb; the marker must still land on a
        // defined center inside the track rather than outside it.
        let narrow = TrackRect {
            left: 10.0,
            right: 18.0,
            top: 0.0,
            bottom: 8.0,
        };
        for muted in [false, true] {
            let g = rail_geometry(&rail(50, muted), narrow, 6.0, 6.0);
            let (cx, cy) = g.marker.center();
            assert!(approx_eq(cx, 14.0), "muted {muted}: {cx}");
            assert!(approx_eq(cy, 4.0), "muted {muted}: {cy}");
            assert!(cx >= narrow.left && cx <= narrow.right);
        }
    }

    // --- marker shape ---------------------------------------------------------

    #[test]
    fn muted_uses_diamond_at_the_same_center_as_the_thumb() {
        for p in [0u8, 50, 100] {
            let normal = geometry(&rail(p, false));
            let muted = geometry(&rail(p, true));
            let MarkerGeometry::Thumb {
                center_x: tx,
                center_y: ty,
                ..
            } = normal.marker
            else {
                panic!("normal at {p}% must be a thumb");
            };
            let MarkerGeometry::MutedDiamond {
                center_x: dx,
                center_y: dy,
                half_size,
            } = muted.marker
            else {
                panic!("muted at {p}% must be a diamond");
            };
            assert_eq!(tx, dx, "same center_x at {p}%");
            assert_eq!(ty, dy, "same center_y at {p}%");
            assert_eq!(half_size, 6.0);
        }
    }

    #[test]
    fn muted_marker_shape_differs_from_thumb_with_defined_centers() {
        // High contrast collapses colors; the muted marker must remain
        // distinguishable by shape alone, and every variant must carry a
        // defined (finite) center for the renderer to paint.
        for p in [0u8, 33, 100] {
            let normal = geometry(&rail(p, false));
            let muted = geometry(&rail(p, true));
            assert!(
                !matches!(normal.marker, MarkerGeometry::MutedDiamond { .. }),
                "normal at {p}% must not render as a diamond"
            );
            assert!(
                !matches!(muted.marker, MarkerGeometry::Thumb { .. }),
                "muted at {p}% must not render as a thumb"
            );
            let (nx, ny) = normal.marker.center();
            let (mx, my) = muted.marker.center();
            assert!(nx.is_finite() && ny.is_finite(), "normal center at {p}%");
            assert!(mx.is_finite() && my.is_finite(), "muted center at {p}%");
            assert_eq!(nx, mx);
            assert_eq!(ny, my);
        }
    }

    // --- threshold bands --------------------------------------------------------

    #[test]
    fn fill_color_matches_core_volume_color_rgb() {
        // Representative percents (5/50/95), plus the 0% and band boundaries
        // (40/41/75/76) — proves the rail does not fork the palette. Uses the
        // VolumePro default thresholds (the same values `Config::default()`
        // seeds), mirroring `core::volume_color_rgb` exactly.
        let cfg = ColorThresholds {
            green_up_to: 40,
            blue_up_to: 75,
            orange_up_to: 100,
        };
        for percent in [0u8, 5, 40, 41, 50, 75, 76, 95, 100] {
            for muted in [false, true] {
                let got = rail(percent, muted).fill_color();
                let (r, g, b) = volume_color_rgb(percent, muted, &cfg);
                assert_eq!(
                    got,
                    Rgba::from_rgb(r, g, b),
                    "percent {percent}, muted {muted}"
                );
            }
        }
    }

    #[test]
    fn fill_color_uses_the_rails_own_threshold_palette() {
        let custom = VolumeThresholdColors {
            muted: Rgba::from_rgb(1, 2, 3),
            low: Rgba::from_rgb(4, 5, 6),
            medium: Rgba::from_rgb(7, 8, 9),
            high: Rgba::from_rgb(10, 11, 12),
        };
        assert_eq!(SignalRail::new(0, false, custom).fill_color(), custom.muted);
        assert_eq!(SignalRail::new(5, false, custom).fill_color(), custom.low);
        assert_eq!(
            SignalRail::new(50, false, custom).fill_color(),
            custom.medium
        );
        assert_eq!(SignalRail::new(95, false, custom).fill_color(), custom.high);
        assert_eq!(SignalRail::new(50, true, custom).fill_color(), custom.muted);
    }

    // --- keyboard mapping ----------------------------------------------------------

    #[test]
    fn keyboard_mapping_home_end_and_arrows_with_configured_step() {
        let r = rail(50, false);
        assert_eq!(rail_apply_key(&r, RailKey::Home, 2).percent, 0);
        assert_eq!(rail_apply_key(&r, RailKey::End, 2).percent, 100);
        assert_eq!(rail_apply_key(&r, RailKey::ArrowUp, 2).percent, 52);
        assert_eq!(rail_apply_key(&r, RailKey::ArrowDown, 2).percent, 48);
        // The step is caller-configured; larger steps apply the same way.
        assert_eq!(rail_apply_key(&r, RailKey::ArrowUp, 10).percent, 60);
        assert_eq!(rail_apply_key(&r, RailKey::ArrowDown, 10).percent, 40);
    }

    #[test]
    fn keyboard_steps_beyond_bounds_clamp() {
        let near_top = rail(99, false);
        assert_eq!(rail_apply_key(&near_top, RailKey::ArrowUp, 2).percent, 100);
        assert_eq!(near_top.step_up(255).percent, 100);
        let near_bottom = rail(1, false);
        assert_eq!(
            rail_apply_key(&near_bottom, RailKey::ArrowDown, 2).percent,
            0
        );
        assert_eq!(near_bottom.step_down(255).percent, 0);
        // Home/End clamp regardless of the starting percent.
        assert_eq!(rail_apply_key(&rail(0, false), RailKey::Home, 2).percent, 0);
        assert_eq!(
            rail_apply_key(&rail(100, false), RailKey::End, 2).percent,
            100
        );
    }

    #[test]
    fn steps_and_keys_preserve_mute_and_thresholds() {
        let muted = rail(50, true);
        assert!(muted.step_up(2).muted);
        assert!(muted.step_down(2).muted);
        assert!(muted.home().muted);
        assert!(muted.end().muted);
        assert_eq!(
            muted.step_up(2).thresholds,
            VolumeThresholdColors::default()
        );
        assert_eq!(
            rail_apply_key(&muted, RailKey::ArrowDown, 2).thresholds,
            VolumeThresholdColors::default()
        );
    }
}
