//! Closed-form analytic harmonic springs and bounded fluid dynamics.

pub mod fluid;
pub mod policy;
pub mod spring;

#[cfg(test)]
pub mod tests;

pub use fluid::{GravityBias, InteractionRipple, PointerStretch};
pub use policy::MotionPolicy;
pub use spring::{AnalyticSpring, SpringConfig};
