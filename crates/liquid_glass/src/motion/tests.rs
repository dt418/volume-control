//! Unit tests for closed-form analytic springs, bounded fluid effects, and motion policy.

use std::time::{Duration, Instant};

use crate::motion::fluid::{InteractionRipple, PointerStretch};
use crate::motion::policy::MotionPolicy;
use crate::motion::spring::{AnalyticSpring, SpringConfig};
use crate::scene::geometry::Point;

#[test]
fn spring_evaluates_at_start_and_settles_at_target() {
    let now = Instant::now();
    let spring = AnalyticSpring::new(0.0, 100.0, SpringConfig::responsive(), now);

    // Initial boundary check
    let (p0, v0) = spring.eval(now);
    assert!((p0 - 0.0).abs() < 1e-4);
    assert!((v0 - 0.0).abs() < 1e-4);
    assert!(!spring.is_settled(now));

    // After 1 second, a 24 rad/s spring should be settled at target 100.0
    let later = now + Duration::from_millis(1000);
    assert!(
        spring.is_settled(later),
        "Spring should be settled after 1s"
    );
    assert_eq!(spring.value(later), 100.0);
}

#[test]
fn spring_retarget_preserves_position_and_velocity() {
    let now = Instant::now();
    let mut spring = AnalyticSpring::new(0.0, 100.0, SpringConfig::responsive(), now);

    // Sample mid-animation at 100ms
    let t_mid = now + Duration::from_millis(100);
    let (p_mid, v_mid) = spring.eval(t_mid);

    // Retarget to 50.0 at t_mid
    spring.retarget(50.0, t_mid);

    // Immediate re-evaluation at t_mid must equal p_mid and v_mid (C1 continuity)
    let (p_new, v_new) = spring.eval(t_mid);
    assert!(
        (p_new - p_mid).abs() < 1e-4,
        "Position jump on retarget: {} vs {}",
        p_new,
        p_mid
    );
    assert!(
        (v_new - v_mid).abs() < 1e-4,
        "Velocity jump on retarget: {} vs {}",
        v_new,
        v_mid
    );
}

#[test]
fn pointer_stretch_clamping_and_smoothing() {
    let now = Instant::now();
    let mut stretch = PointerStretch::new(Point::new(10.0, 10.0), now);

    // Rapid pointer movement simulating 2000 px/s drag
    let t1 = now + Duration::from_millis(16);
    stretch.update(Point::new(42.0, 10.0), t1);

    let offset = stretch.stretch_offset();
    assert!(offset.x > 0.0);
    assert!(
        offset.x <= 8.0,
        "Stretch offset must not exceed max clamp limit"
    );

    stretch.release();
    assert_eq!(stretch.stretch_offset(), Point::ZERO);
}

#[test]
fn interaction_ripple_lifecycle() {
    let now = Instant::now();
    let ripple = InteractionRipple::new(Point::new(50.0, 50.0), now);

    assert!(!ripple.is_finished(now));
    let (r0, a0) = ripple.evaluate(now);
    assert_eq!(r0, 0.0);
    assert!(a0 > 0.0);

    let t_done = now + Duration::from_millis(400);
    assert!(ripple.is_finished(t_done));
    let (_, a_done) = ripple.evaluate(t_done);
    assert_eq!(a_done, 0.0);
}

#[test]
fn motion_policy_reduced_motion_enforcement() {
    let offset = Point::new(6.0, 4.0);
    assert_eq!(
        MotionPolicy::effective_stretch(offset, true),
        Point::ZERO,
        "Reduced motion must zero out stretch"
    );
    assert_eq!(MotionPolicy::effective_stretch(offset, false), offset);

    assert_eq!(
        MotionPolicy::effective_ripple_intensity(0.25, true),
        0.0,
        "Reduced motion must zero out ripple"
    );
    assert_eq!(MotionPolicy::effective_ripple_intensity(0.25, false), 0.25);

    let config = MotionPolicy::select_spring_config(true);
    assert_eq!(
        config.damping_ratio, 1.0,
        "Reduced motion requires critical damping (no bounce)"
    );
}
