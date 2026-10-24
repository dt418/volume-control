//! Surface open/close planning: authoritative visibility -> window operations.

use volumectl::ui::{AppState, SurfaceId};

/// One window lifecycle operation derived from a visibility diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceOp {
    /// Create and show the surface window.
    Open(SurfaceId),
    /// Hide (and drop) the surface window.
    Close(SurfaceId),
}

/// Plan window operations that bring the open set in line with host visibility.
///
/// Closes are emitted before opens so window churn never overlaps two
/// competing surfaces of the same kind. Within each group, operations follow
/// the canonical [`SurfaceId`] declaration order, keeping the plan
/// deterministic across platforms.
#[must_use]
pub fn plan(open: &[SurfaceId], state: &AppState) -> Vec<SurfaceOp> {
    // Tray is a native OS tray, never a glass window.
    const ALL: [SurfaceId; 4] = [
        SurfaceId::Overlay,
        SurfaceId::Mixer,
        SurfaceId::Settings,
        SurfaceId::Help,
    ];

    let mut ops = Vec::new();
    for surface in ALL {
        if !state.is_visible(surface) && open.contains(&surface) {
            ops.push(SurfaceOp::Close(surface));
        }
    }
    for surface in ALL {
        if state.is_visible(surface) && !open.contains(&surface) {
            ops.push(SurfaceOp::Open(surface));
        }
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(visible: &[SurfaceId]) -> AppState {
        let mut state = AppState::default();
        for surface in visible {
            state.show(*surface);
        }
        state
    }

    #[test]
    fn tray_is_never_planned_as_a_window() {
        let state = state_with(&[SurfaceId::Tray]);
        assert_eq!(plan(&[], &state), Vec::new());
    }

    #[test]
    fn empty_plan_when_open_set_matches_visibility() {
        let state = state_with(&[SurfaceId::Mixer, SurfaceId::Help]);
        let open = [SurfaceId::Mixer, SurfaceId::Help];
        assert_eq!(plan(&open, &state), Vec::new());
    }

    #[test]
    fn newly_visible_surface_plans_open() {
        let state = state_with(&[SurfaceId::Settings]);
        assert_eq!(
            plan(&[], &state),
            vec![SurfaceOp::Open(SurfaceId::Settings)]
        );
    }

    #[test]
    fn hidden_surface_plans_close() {
        let state = AppState::default();
        let open = [SurfaceId::Overlay, SurfaceId::Mixer];
        assert_eq!(
            plan(&open, &state),
            vec![
                SurfaceOp::Close(SurfaceId::Overlay),
                SurfaceOp::Close(SurfaceId::Mixer),
            ]
        );
    }

    #[test]
    fn closes_precede_opens_in_canonical_order() {
        let state = state_with(&[SurfaceId::Help, SurfaceId::Mixer]);
        let open = [SurfaceId::Overlay, SurfaceId::Settings];
        assert_eq!(
            plan(&open, &state),
            vec![
                SurfaceOp::Close(SurfaceId::Overlay),
                SurfaceOp::Close(SurfaceId::Settings),
                SurfaceOp::Open(SurfaceId::Mixer),
                SurfaceOp::Open(SurfaceId::Help),
            ]
        );
    }
}
