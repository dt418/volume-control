//! Closed-form analytic harmonic oscillator spring physics with exact velocity continuity.

use std::time::Instant;

/// Spring configuration specifying damping ratio and natural angular frequency.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringConfig {
    /// Damping ratio: < 1.0 underdamped (bouncy), 1.0 critically damped, > 1.0 overdamped.
    pub damping_ratio: f32,
    /// Natural angular frequency in radians per second.
    pub angular_frequency: f32,
    /// Absolute position tolerance for declaring the spring settled.
    pub position_tolerance: f32,
    /// Absolute velocity tolerance for declaring the spring settled.
    pub velocity_tolerance: f32,
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            damping_ratio: 0.75,
            angular_frequency: 18.0,
            position_tolerance: 0.001,
            velocity_tolerance: 0.01,
        }
    }
}

impl SpringConfig {
    /// Stiff, responsive spring with subtle overshoot (good for HUD volume sliders).
    pub const fn responsive() -> Self {
        Self {
            damping_ratio: 0.72,
            angular_frequency: 24.0,
            position_tolerance: 0.001,
            velocity_tolerance: 0.01,
        }
    }

    /// Critically damped spring with zero bounce (smooth deceleration).
    pub const fn without_bounce() -> Self {
        Self {
            damping_ratio: 1.0,
            angular_frequency: 20.0,
            position_tolerance: 0.001,
            velocity_tolerance: 0.01,
        }
    }
}

/// Evaluates spring motion analytically at arbitrary time t without numerical integration.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticSpring {
    config: SpringConfig,
    start_time: Instant,
    initial_pos: f32,
    initial_vel: f32,
    target: f32,
}

impl AnalyticSpring {
    /// Create a new spring starting at initial position and target.
    pub fn new(initial_pos: f32, target: f32, config: SpringConfig, now: Instant) -> Self {
        Self {
            config,
            start_time: now,
            initial_pos,
            initial_vel: 0.0,
            target,
        }
    }

    /// Retarget the spring to a new target while preserving instantaneous position and velocity.
    pub fn retarget(&mut self, new_target: f32, now: Instant) {
        if (new_target - self.target).abs() < 1e-6 {
            return;
        }

        let (current_pos, current_vel) = self.eval(now);
        self.initial_pos = current_pos;
        self.initial_vel = current_vel;
        self.target = new_target;
        self.start_time = now;
    }

    /// Instantaneous position and velocity at the specified timestamp.
    pub fn eval(&self, now: Instant) -> (f32, f32) {
        let t = now.saturating_duration_since(self.start_time).as_secs_f32();
        let zeta = self.config.damping_ratio;
        let omega_n = self.config.angular_frequency;
        let x_rel = self.initial_pos - self.target;
        let v0 = self.initial_vel;

        if (zeta - 1.0).abs() < 1e-5 {
            // Critically damped (zeta == 1.0)
            let decay = (-omega_n * t).exp();
            let c2 = v0 + omega_n * x_rel;
            let pos = decay * (x_rel + c2 * t) + self.target;
            let vel = decay * (v0 - omega_n * c2 * t);
            (pos, vel)
        } else if zeta < 1.0 {
            // Underdamped (zeta < 1.0)
            let omega_d = omega_n * (1.0 - zeta * zeta).sqrt();
            let decay = (-zeta * omega_n * t).exp();
            let cos_term = (omega_d * t).cos();
            let sin_term = (omega_d * t).sin();
            let c2 = (v0 + zeta * omega_n * x_rel) / omega_d;

            let pos = decay * (x_rel * cos_term + c2 * sin_term) + self.target;
            let vel = decay
                * (v0 * cos_term
                    - ((zeta * omega_n * v0 + omega_n * omega_n * x_rel) / omega_d) * sin_term);
            (pos, vel)
        } else {
            // Overdamped (zeta > 1.0)
            let gamma = omega_n * (zeta * zeta - 1.0).sqrt();
            let r1 = -zeta * omega_n + gamma;
            let r2 = -zeta * omega_n - gamma;
            let c2 = (v0 - r1 * x_rel) / (r2 - r1);
            let c1 = x_rel - c2;

            let pos = c1 * (r1 * t).exp() + c2 * (r2 * t).exp() + self.target;
            let vel = c1 * r1 * (r1 * t).exp() + c2 * r2 * (r2 * t).exp();
            (pos, vel)
        }
    }

    /// Evaluated position clamped to target if settled.
    pub fn value(&self, now: Instant) -> f32 {
        if self.is_settled(now) {
            self.target
        } else {
            self.eval(now).0
        }
    }

    /// True if the spring is within position and velocity tolerances.
    pub fn is_settled(&self, now: Instant) -> bool {
        let (pos, vel) = self.eval(now);
        (pos - self.target).abs() <= self.config.position_tolerance
            && vel.abs() <= self.config.velocity_tolerance
    }

    pub fn target(&self) -> f32 {
        self.target
    }
}
