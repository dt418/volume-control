//! Bounded liquid visual effects: pointer velocity stretch, interaction ripples, and gravity bias.

use std::time::Instant;

use crate::scene::geometry::Point;

/// Calculates pointer drag velocity and elastic visual stretch factors.
#[derive(Debug, Clone)]
pub struct PointerStretch {
    last_position: Point,
    last_time: Instant,
    velocity: Point,
    max_stretch_offset: f32,
}

impl PointerStretch {
    pub fn new(initial_pos: Point, now: Instant) -> Self {
        Self {
            last_position: initial_pos,
            last_time: now,
            velocity: Point::ZERO,
            max_stretch_offset: 8.0, // Maximum 8 logical px deformation
        }
    }

    /// Update pointer position and compute instantaneous smoothed velocity.
    pub fn update(&mut self, new_pos: Point, now: Instant) {
        let dt = now.saturating_duration_since(self.last_time).as_secs_f32();
        if dt > 1e-4 {
            let vx = (new_pos.x - self.last_position.x) / dt;
            let vy = (new_pos.y - self.last_position.y) / dt;
            // Exponential moving average filter
            self.velocity.x = self.velocity.x * 0.6 + vx * 0.4;
            self.velocity.y = self.velocity.y * 0.6 + vy * 0.4;
            self.last_position = new_pos;
            self.last_time = now;
        }
    }

    /// Calculate visual trailing displacement offset (clamped to max bounds).
    pub fn stretch_offset(&self) -> Point {
        let k = 0.008; // Velocity to pixel offset factor
        Point {
            x: (self.velocity.x * k).clamp(-self.max_stretch_offset, self.max_stretch_offset),
            y: (self.velocity.y * k).clamp(-self.max_stretch_offset, self.max_stretch_offset),
        }
    }

    /// Reset velocity to zero when pointer is released.
    pub fn release(&mut self) {
        self.velocity = Point::ZERO;
    }
}

/// Expanding radial ripple wave triggered by pointer click or tap.
#[derive(Debug, Clone)]
pub struct InteractionRipple {
    pub origin: Point,
    pub start_time: Instant,
    pub duration_secs: f32,
    pub max_radius: f32,
    pub max_amplitude: f32,
}

impl InteractionRipple {
    pub fn new(origin: Point, now: Instant) -> Self {
        Self {
            origin,
            start_time: now,
            duration_secs: 0.35, // 350ms duration
            max_radius: 80.0,    // 80 logical px radius
            max_amplitude: 0.25, // 25% max intensity
        }
    }

    /// Current radius and opacity factor `(radius, intensity)` at timestamp `now`.
    pub fn evaluate(&self, now: Instant) -> (f32, f32) {
        let elapsed = now.saturating_duration_since(self.start_time).as_secs_f32();
        if elapsed >= self.duration_secs {
            return (self.max_radius, 0.0);
        }

        let progress = elapsed / self.duration_secs;
        let radius = self.max_radius * (1.0 - (-3.0 * progress).exp());
        let intensity = self.max_amplitude * (1.0 - progress);

        (radius, intensity)
    }

    /// True if the ripple has finished expanding and faded out.
    pub fn is_finished(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start_time).as_secs_f32() >= self.duration_secs
    }
}

/// Subtle downward gravity lighting bias for floating cards.
pub struct GravityBias;

impl GravityBias {
    /// Compute subtle downward light and shadow offset (in logical pixels).
    pub const fn downward_bias_px() -> f32 {
        1.5
    }
}
