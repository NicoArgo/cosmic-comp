use cosmic_settings_config::shortcuts::action::Direction;
use smithay::utils::{Logical, Point};
use std::{collections::VecDeque, time::Duration};
use tracing::trace;

const HISTORY_LIMIT: Duration = Duration::from_millis(150);
const DECELERATION_TOUCHPAD: f64 = 0.997;

#[derive(Debug, Clone, Copy)]
pub struct SwipeEvent {
    delta: f64,
    timestamp: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeAction {
    NextWorkspace,
    PrevWorkspace,
    /// Focus the window to the right / to the left of the current one.
    NextWindow,
    PrevWindow,
    /// Open the workspace overview.
    Overview,
    /// Put every window away, or bring back the ones that were put away.
    ShowDesktop,
}

impl SwipeAction {
    /// Whether the action follows the fingers continuously.
    ///
    /// Workspace switching is animated along with the swipe and committed when
    /// the fingers lift. Everything else happens once, at a point in the
    /// gesture, and must not be re-fired on every motion event.
    pub fn is_continuous(self) -> bool {
        matches!(self, Self::NextWorkspace | Self::PrevWorkspace)
    }
}

/// What a three-finger swipe in `direction` means.
///
/// Sideways follows `natural_scroll`, because moving between windows is moving
/// along a row and reads like dragging content. Up and down deliberately do
/// not: they are two unrelated commands rather than two ends of an axis, and
/// swapping "show me everything" with "hide everything" because of a scrolling
/// preference would be baffling.
pub fn three_finger_action(direction: Direction, natural_scroll: bool) -> Option<SwipeAction> {
    Some(match direction {
        Direction::Left => {
            if natural_scroll {
                SwipeAction::NextWindow
            } else {
                SwipeAction::PrevWindow
            }
        }
        Direction::Right => {
            if natural_scroll {
                SwipeAction::PrevWindow
            } else {
                SwipeAction::NextWindow
            }
        }
        Direction::Up => SwipeAction::Overview,
        Direction::Down => SwipeAction::ShowDesktop,
    })
}

/// How far a swipe must travel before a one-shot action fires, in the same
/// units as the accumulated delta.
///
/// The direction is decided from the very first motion event, which can be a
/// pixel or two. Acting there would turn a resting hand into a window switch,
/// so a one-shot action waits until the gesture is unmistakably a gesture.
pub const DISCRETE_THRESHOLD: f64 = 60.0;

#[derive(Debug, Clone)]
pub struct GestureState {
    pub fingers: u32,
    pub direction: Option<Direction>,
    pub action: Option<SwipeAction>,
    /// Whether a one-shot action has already fired for this gesture. Without
    /// it, holding past the threshold would repeat the action on every motion
    /// event for as long as the fingers keep moving.
    pub triggered: bool,
    pub delta: f64,
    // Delta tracking inspired by Niri (GPL-3.0) https://github.com/YaLTeR/niri/tree/v0.1.3
    pub history: VecDeque<SwipeEvent>,
}

impl GestureState {
    pub fn new(fingers: u32) -> Self {
        GestureState {
            fingers,
            direction: None,
            action: None,
            triggered: false,
            delta: 0.0,
            history: VecDeque::new(),
        }
    }

    pub fn update(&mut self, movement: Point<f64, Logical>, timestamp: Duration) -> bool {
        let first_update = self.direction.is_none();

        if first_update {
            // Find proper direction
            self.direction = if movement.x.abs() > movement.y.abs() {
                if movement.x > 0.0 {
                    Some(Direction::Right)
                } else {
                    Some(Direction::Left)
                }
            } else if movement.y > 0.0 {
                Some(Direction::Down)
            } else {
                Some(Direction::Up)
            }
        }

        let delta = match self.direction {
            Some(Direction::Left) => -movement.x,
            Some(Direction::Right) => movement.x,
            Some(Direction::Up) => -movement.y,
            Some(Direction::Down) => movement.y,
            None => 0.0,
        };

        self.push(delta, timestamp);
        first_update
    }

    /// Pushes a new reading into the tracker.
    fn push(&mut self, delta: f64, timestamp: Duration) {
        // For the events that we care about, timestamps should always increase
        // monotonically.
        if let Some(last) = self.history.back()
            && timestamp < last.timestamp
        {
            trace!(
                "ignoring event with timestamp {timestamp:?} earlier than last {:?}",
                last.timestamp
            );
            return;
        }

        self.history.push_back(SwipeEvent { delta, timestamp });
        self.delta += delta;

        self.trim_history();
    }

    /// Computes the current gesture velocity.
    pub fn velocity(&self) -> f64 {
        let (Some(first), Some(last)) = (self.history.front(), self.history.back()) else {
            return 0.;
        };

        let total_time = (last.timestamp - first.timestamp).as_secs_f64();
        if total_time == 0. {
            return 0.;
        }

        let total_delta = self.history.iter().map(|event| event.delta).sum::<f64>();
        total_delta / total_time
    }

    /// Computes the gesture end position after decelerating to a halt.
    pub fn projected_end_pos(&self) -> f64 {
        let vel = self.velocity();
        self.delta - vel / (1000. * DECELERATION_TOUCHPAD.ln())
    }

    fn trim_history(&mut self) {
        let Some(&SwipeEvent { timestamp, .. }) = self.history.back() else {
            return;
        };

        while let Some(first) = self.history.front() {
            if timestamp <= first.timestamp + HISTORY_LIMIT {
                break;
            }

            let _ = self.history.pop_front();
        }
    }
}

impl Default for GestureState {
    fn default() -> Self {
        GestureState::new(0)
    }
}

// rubber_band.rs from Niri (GPL-3.0) https://github.com/YaLTeR/niri/blob/db49deb7fd2fbe805ceec060aa4dec65009ad7a7/src/rubber_band.rs
#[derive(Debug, Clone, Copy)]
pub struct RubberBand {
    pub stiffness: f64,
    pub limit: f64,
}

impl RubberBand {
    pub fn band(&self, x: f64) -> f64 {
        let c = self.stiffness;
        let d = self.limit;

        (1. - (1. / (x * c / d + 1.))) * d
    }

    pub fn derivative(&self, x: f64) -> f64 {
        let c = self.stiffness;
        let d = self.limit;

        c * d * d / (c * x + d).powi(2)
    }

    pub fn clamp(&self, min: f64, max: f64, x: f64) -> f64 {
        let clamped = x.clamp(min, max);
        let sign = if x < clamped { -1. } else { 1. };
        let diff = (x - clamped).abs();

        clamped + sign * self.band(diff)
    }

    pub fn clamp_derivative(&self, min: f64, max: f64, x: f64) -> f64 {
        if min <= x && x <= max {
            return 1.;
        }

        let clamped = x.clamp(min, max);
        let diff = (x - clamped).abs();
        self.derivative(diff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sideways_swipes_follow_natural_scrolling() {
        assert_eq!(
            three_finger_action(Direction::Right, false),
            Some(SwipeAction::NextWindow)
        );
        assert_eq!(
            three_finger_action(Direction::Right, true),
            Some(SwipeAction::PrevWindow)
        );
        assert_eq!(
            three_finger_action(Direction::Left, false),
            Some(SwipeAction::PrevWindow)
        );
        assert_eq!(
            three_finger_action(Direction::Left, true),
            Some(SwipeAction::NextWindow)
        );
    }

    #[test]
    fn vertical_swipes_are_not_mirrored_by_natural_scrolling() {
        // Guards a plausible "fix": someone noticing the asymmetry above and
        // making up and down invert too. Overview and show-desktop are separate
        // commands, not two ends of an axis — flipping them under a scrolling
        // preference would hide every window when the user asked to see them.
        for natural_scroll in [false, true] {
            assert_eq!(
                three_finger_action(Direction::Up, natural_scroll),
                Some(SwipeAction::Overview)
            );
            assert_eq!(
                three_finger_action(Direction::Down, natural_scroll),
                Some(SwipeAction::ShowDesktop)
            );
        }
    }

    #[test]
    fn only_workspace_switching_follows_the_fingers() {
        // Everything else fires once, past a threshold. Getting this backwards
        // would repeat the action on every motion event of the swipe.
        assert!(SwipeAction::NextWorkspace.is_continuous());
        assert!(SwipeAction::PrevWorkspace.is_continuous());
        assert!(!SwipeAction::NextWindow.is_continuous());
        assert!(!SwipeAction::PrevWindow.is_continuous());
        assert!(!SwipeAction::Overview.is_continuous());
        assert!(!SwipeAction::ShowDesktop.is_continuous());
    }
}
