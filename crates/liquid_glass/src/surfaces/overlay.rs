//! Native Bottom-Right Overlay surface (Volume HUD card).

use std::time::{Duration, Instant};

use crate::material::MaterialRole;
use crate::motion::policy::MotionPolicy;
use crate::motion::spring::{AnalyticSpring, SpringConfig};
use crate::scene::geometry::Rect;
use crate::scene::graph::Scene;
use crate::scene::node::{SceneNode, SemanticId};

pub const OVERLAY_WIDTH: f32 = 336.0;
pub const OVERLAY_HEIGHT: f32 = 88.0;

pub const OVERLAY_ROOT_ID: SemanticId = SemanticId(100);
pub const GAUGE_TRACK_ID: SemanticId = SemanticId(101);
pub const GAUGE_FILL_ID: SemanticId = SemanticId(102);
pub const PERCENT_TEXT_ID: SemanticId = SemanticId(103);
pub const DEVICE_TEXT_ID: SemanticId = SemanticId(104);
pub const NOTIFICATION_TEXT_ID: SemanticId = SemanticId(106);

/// Manages state, spring animations, auto-hide lifecycle, and scene generation for the Volume Overlay HUD.
pub struct OverlaySurface {
    volume_percent: u8,
    muted: bool,
    device_name: String,
    volume_spring: AnalyticSpring,
    opacity_spring: AnalyticSpring,
    last_show_time: Instant,
    notification_text: Option<String>,
    auto_hide_duration: Duration,
    is_visible: bool,
}

impl OverlaySurface {
    /// Create a new Overlay surface initialized at the given time.
    pub fn new(now: Instant) -> Self {
        Self {
            volume_percent: 50,
            muted: false,
            device_name: "Default Audio Device".into(),
            volume_spring: AnalyticSpring::new(50.0, 50.0, SpringConfig::responsive(), now),
            opacity_spring: AnalyticSpring::new(0.0, 0.0, SpringConfig::without_bounce(), now),
            last_show_time: now,
            notification_text: None,
            auto_hide_duration: Duration::from_millis(1500),
            is_visible: false,
        }
    }

    /// Trigger the overlay to display or refresh on volume change.
    pub fn show(
        &mut self,
        volume_percent: u8,
        muted: bool,
        device: Option<&str>,
        now: Instant,
        reduce_motion: bool,
    ) {
        self.volume_percent = volume_percent.min(100);
        self.muted = muted;
        self.notification_text = None;
        if let Some(dev) = device {
            self.device_name = dev.to_string();
        }

        self.last_show_time = now;
        self.auto_hide_duration = Duration::from_millis(1500);
        self.is_visible = true;

        let spring_cfg = MotionPolicy::select_spring_config(reduce_motion);
        self.volume_spring = AnalyticSpring::new(
            self.volume_spring.value(now),
            self.volume_percent as f32,
            spring_cfg,
            now,
        );

        // Retarget opacity to 1.0 (fully visible)
        self.opacity_spring.retarget(1.0, now);
    }

    /// Trigger the overlay to display an arbitrary text toast notification (e.g. updates, config reload).
    pub fn show_notification(&mut self, text: impl Into<String>, now: Instant) {
        self.notification_text = Some(text.into());
        self.last_show_time = now;
        self.auto_hide_duration = Duration::from_millis(2500); // 2.5s for reading
        self.is_visible = true;
        self.opacity_spring.retarget(1.0, now);
    }

    pub fn notification_text(&self) -> Option<&str> {
        self.notification_text.as_deref()
    }

    /// Update internal timer and spring physics. Returns `true` if an animation is actively running.
    pub fn update(&mut self, now: Instant) -> bool {
        if !self.is_visible {
            return false;
        }

        // Check auto-hide timer expiration
        let elapsed = now.saturating_duration_since(self.last_show_time);
        if elapsed >= self.auto_hide_duration && self.opacity_spring.target() > 0.0 {
            let timeout_time = self.last_show_time + self.auto_hide_duration;
            self.opacity_spring.retarget(0.0, timeout_time);
        }

        let opacity = self.opacity_spring.value(now);
        if opacity <= 0.001
            && self.opacity_spring.is_settled(now)
            && elapsed >= self.auto_hide_duration
        {
            self.is_visible = false;
            return false;
        }

        // Active if either volume spring or opacity spring is not settled
        !self.volume_spring.is_settled(now) || !self.opacity_spring.is_settled(now)
    }

    /// True if the overlay HUD is currently visible and should be painted.
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Populate or update the semantic scene nodes for the overlay surface.
    pub fn populate_scene(&self, scene: &mut Scene, now: Instant) {
        if !self.is_visible {
            // Hide or remove nodes if hidden
            if let Some(root) = scene.get_mut(OVERLAY_ROOT_ID) {
                root.visible = false;
            }
            return;
        }

        let current_vol = self.volume_spring.value(now).clamp(0.0, 100.0);

        // 1. Root HUD glass card (336 x 88)
        let mut root_node = SceneNode::new(
            OVERLAY_ROOT_ID,
            MaterialRole::Hud,
            Rect::new(0.0, 0.0, OVERLAY_WIDTH, OVERLAY_HEIGHT),
            0,
        );
        root_node.visible = true;
        scene.insert(root_node);

        if let Some(ref msg) = self.notification_text {
            let notif_node = SceneNode::new(
                NOTIFICATION_TEXT_ID,
                MaterialRole::Hud,
                Rect::new(24.0, 24.0, 288.0, 40.0),
                1,
            )
            .with_text(msg);
            scene.insert(notif_node);
            scene.add_child(OVERLAY_ROOT_ID, NOTIFICATION_TEXT_ID);
        } else {
            let volume_text = if self.muted {
                "Muted".to_string()
            } else {
                format!("{}%", self.volume_percent)
            };
            let percent_node = SceneNode::new(
                PERCENT_TEXT_ID,
                MaterialRole::Hud,
                Rect::new(24.0, 18.0, 80.0, 24.0),
                1,
            )
            .with_text(volume_text);

            let device_node = SceneNode::new(
                DEVICE_TEXT_ID,
                MaterialRole::Hud,
                Rect::new(110.0, 20.0, 202.0, 20.0),
                1,
            )
            .with_text(&self.device_name);

            let track_node = SceneNode::new(
                GAUGE_TRACK_ID,
                MaterialRole::Control,
                Rect::new(24.0, 52.0, 288.0, 14.0),
                1,
            );

            let fill_width = if self.muted {
                0.0
            } else {
                288.0 * (current_vol / 100.0)
            };
            let fill_node = SceneNode::new(
                GAUGE_FILL_ID,
                MaterialRole::Accent,
                Rect::new(24.0, 52.0, fill_width, 14.0),
                2,
            );

            scene.insert(percent_node);
            scene.insert(device_node);
            scene.insert(track_node);
            scene.insert(fill_node);

            scene.add_child(OVERLAY_ROOT_ID, PERCENT_TEXT_ID);
            scene.add_child(OVERLAY_ROOT_ID, DEVICE_TEXT_ID);
            scene.add_child(OVERLAY_ROOT_ID, GAUGE_TRACK_ID);
            scene.add_child(GAUGE_TRACK_ID, GAUGE_FILL_ID);
        }
    }

    pub fn volume_percent(&self) -> u8 {
        self.volume_percent
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }
}
