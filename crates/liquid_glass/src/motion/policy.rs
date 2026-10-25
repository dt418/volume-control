//! Motion policy and reduced-motion accessibility enforcement.

use crate::motion::spring::SpringConfig;
use crate::scene::geometry::Point;

/// Policy helper governing animation behavior under standard vs reduced-motion settings.
pub struct MotionPolicy;

impl MotionPolicy {
    /// True if fluid elastic stretch, trailing, and ripple waves are permitted.
    pub const fn allow_fluid_deformation(reduce_motion: bool) -> bool {
        !reduce_motion
    }

    /// Select spring physics config respecting reduced motion.
    pub fn select_spring_config(reduce_motion: bool) -> SpringConfig {
        if reduce_motion {
            SpringConfig::without_bounce()
        } else {
            SpringConfig::responsive()
        }
    }

    /// Calculate effective pointer stretch offset (zeroed under reduced motion).
    pub fn effective_stretch(offset: Point, reduce_motion: bool) -> Point {
        if reduce_motion {
            Point::ZERO
        } else {
            offset
        }
    }

    /// Calculate effective ripple wave intensity (zeroed under reduced motion).
    pub fn effective_ripple_intensity(intensity: f32, reduce_motion: bool) -> f32 {
        if reduce_motion {
            0.0
        } else {
            intensity
        }
    }
}
