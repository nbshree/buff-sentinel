use std::time::{Duration, Instant};

const DEADLINE_CONFIRMATION_GRACE: Duration = Duration::from_millis(600);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelinePhase {
    Stopped,
    Waiting,
    Tracking,
    Prewarning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineAction {
    Triggered,
    PrewarnThree,
    PrewarnTwo,
    PrewarnOne,
    Reset,
}

pub struct BuffTimeline {
    phase: TimelinePhase,
    cycle: Duration,
    expected_at: Option<Instant>,
    sent_three: bool,
    sent_two: bool,
    sent_one: bool,
}

impl BuffTimeline {
    pub fn new(cycle_ms: u64) -> Self {
        Self {
            phase: TimelinePhase::Stopped,
            cycle: Duration::from_millis(cycle_ms),
            expected_at: None,
            sent_three: false,
            sent_two: false,
            sent_one: false,
        }
    }

    pub fn start_waiting(&mut self, cycle_ms: u64) {
        self.cycle = Duration::from_millis(cycle_ms);
        self.phase = TimelinePhase::Waiting;
        self.expected_at = None;
        self.sent_three = false;
        self.sent_two = false;
        self.sent_one = false;
    }

    pub fn stop(&mut self) {
        self.phase = TimelinePhase::Stopped;
        self.expected_at = None;
        self.sent_three = false;
        self.sent_two = false;
        self.sent_one = false;
    }

    pub fn reset_waiting(&mut self) {
        self.phase = TimelinePhase::Waiting;
        self.expected_at = None;
        self.sent_three = false;
        self.sent_two = false;
        self.sent_one = false;
    }

    pub const fn phase(&self) -> TimelinePhase {
        self.phase
    }

    pub const fn expected_at(&self) -> Option<Instant> {
        self.expected_at
    }

    #[cfg(test)]
    pub fn update(&mut self, now: Instant, icon_present: bool) -> Vec<TimelineAction> {
        self.update_with_detected_at(now, icon_present, None)
    }

    pub fn update_with_detected_at(
        &mut self,
        now: Instant,
        icon_present: bool,
        detected_at: Option<Instant>,
    ) -> Vec<TimelineAction> {
        match self.phase {
            TimelinePhase::Stopped => Vec::new(),
            TimelinePhase::Waiting => {
                if icon_present {
                    self.anchor(valid_detection_time(now, detected_at));
                    vec![TimelineAction::Triggered]
                } else {
                    Vec::new()
                }
            }
            TimelinePhase::Tracking | TimelinePhase::Prewarning => {
                self.update_anchored(now, icon_present, detected_at)
            }
        }
    }

    fn update_anchored(
        &mut self,
        now: Instant,
        icon_present: bool,
        detected_at: Option<Instant>,
    ) -> Vec<TimelineAction> {
        let Some(expected_at) = self.expected_at else {
            self.reset_waiting();
            return vec![TimelineAction::Reset];
        };

        if now >= expected_at {
            if icon_present {
                let detected_near_deadline = detected_at.filter(|detected| {
                    expected_at.saturating_duration_since(*detected) <= Duration::from_secs(1)
                });
                self.anchor(valid_detection_time(now, detected_near_deadline));
                return vec![TimelineAction::Triggered];
            }
            if now < expected_at + DEADLINE_CONFIRMATION_GRACE {
                return Vec::new();
            }
            self.reset_waiting();
            return vec![TimelineAction::Reset];
        }

        let mut actions = Vec::new();
        let remaining = expected_at.saturating_duration_since(now);
        if remaining <= Duration::from_secs(3) && !self.sent_three {
            self.sent_three = true;
            self.phase = TimelinePhase::Prewarning;
            actions.push(TimelineAction::PrewarnThree);
        }
        if remaining <= Duration::from_secs(2) && !self.sent_two {
            self.sent_two = true;
            actions.push(TimelineAction::PrewarnTwo);
        }
        if remaining <= Duration::from_secs(1) && !self.sent_one {
            self.sent_one = true;
            actions.push(TimelineAction::PrewarnOne);
        }

        actions
    }

    fn anchor(&mut self, now: Instant) {
        self.phase = TimelinePhase::Tracking;
        self.expected_at = now.checked_add(self.cycle);
        self.sent_three = false;
        self.sent_two = false;
        self.sent_one = false;
    }
}

fn valid_detection_time(now: Instant, detected_at: Option<Instant>) -> Instant {
    detected_at
        .filter(|detected| *detected <= now)
        .unwrap_or(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_first_real_icon_before_starting_timeline() {
        let start = Instant::now();
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        assert!(timeline.update(start, false).is_empty());
        assert_eq!(timeline.phase(), TimelinePhase::Waiting);
        assert_eq!(
            timeline.update(start + Duration::from_secs(2), true),
            [TimelineAction::Triggered]
        );
        assert_eq!(timeline.phase(), TimelinePhase::Tracking);
    }

    #[test]
    fn ignores_icon_presence_before_the_twenty_second_deadline() {
        let start = Instant::now();
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, true);
        assert!(
            timeline
                .update(start + Duration::from_secs(10), true)
                .is_empty()
        );
        assert_eq!(
            timeline.expected_at(),
            start.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn emits_prealerts_once_and_reanchors_only_at_deadline() {
        let start = Instant::now();
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, true);
        assert_eq!(
            timeline.update(start + Duration::from_secs(17), false),
            [TimelineAction::PrewarnThree]
        );
        assert!(
            timeline
                .update(start + Duration::from_millis(17_500), false)
                .is_empty()
        );
        assert_eq!(
            timeline.update(start + Duration::from_secs(18), false),
            [TimelineAction::PrewarnTwo]
        );
        assert_eq!(
            timeline.update(start + Duration::from_secs(19), false),
            [TimelineAction::PrewarnOne]
        );
        assert_eq!(
            timeline.update(start + Duration::from_secs(20), true),
            [TimelineAction::Triggered]
        );
        assert_eq!(
            timeline.expected_at(),
            start.checked_add(Duration::from_secs(40))
        );
    }

    #[test]
    fn missing_icon_at_deadline_resets_to_waiting_without_future_prediction() {
        let start = Instant::now();
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, true);
        assert!(
            timeline
                .update(start + Duration::from_secs(20), false)
                .is_empty()
        );
        assert_eq!(timeline.phase(), TimelinePhase::Tracking);
        let actions = timeline.update(start + Duration::from_millis(20_600), false);
        assert!(actions.contains(&TimelineAction::Reset));
        assert_eq!(timeline.phase(), TimelinePhase::Waiting);
        assert_eq!(timeline.expected_at(), None);
        assert!(
            timeline
                .update(start + Duration::from_secs(40), false)
                .is_empty()
        );
    }

    #[test]
    fn a_real_icon_after_reset_establishes_a_fresh_timeline() {
        let start = Instant::now();
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, true);
        assert_eq!(
            timeline.update(start + Duration::from_millis(20_600), false),
            [TimelineAction::Reset]
        );

        let detected_again = start + Duration::from_secs(24);
        assert_eq!(
            timeline.update(detected_again, true),
            [TimelineAction::Triggered]
        );
        assert_eq!(
            timeline.expected_at(),
            detected_again.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn confirmation_anchors_the_timeline_at_the_first_matching_frame() {
        let first_match = Instant::now();
        let confirmed_at = first_match + Duration::from_millis(166);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);

        assert_eq!(
            timeline.update_with_detected_at(confirmed_at, true, Some(first_match)),
            [TimelineAction::Triggered]
        );
        assert_eq!(
            timeline.expected_at(),
            first_match.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn next_trigger_uses_a_new_match_observed_just_before_the_deadline() {
        let start = Instant::now();
        let expected = start + Duration::from_secs(20);
        let first_match = expected - Duration::from_millis(100);
        let confirmed_at = expected + Duration::from_millis(70);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, true);

        assert_eq!(
            timeline.update_with_detected_at(confirmed_at, true, Some(first_match)),
            [TimelineAction::Triggered]
        );
        assert_eq!(
            timeline.expected_at(),
            first_match.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn stale_detection_time_is_not_reused_at_the_next_deadline() {
        let start = Instant::now();
        let confirmed_at = start + Duration::from_millis(20_100);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, true);

        timeline.update_with_detected_at(confirmed_at, true, Some(start));
        assert_eq!(
            timeline.expected_at(),
            confirmed_at.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn late_trigger_during_confirmation_grace_does_not_reset_the_timeline() {
        let start = Instant::now();
        let expected = start + Duration::from_secs(20);
        let first_match = expected + Duration::from_millis(180);
        let confirmed_at = expected + Duration::from_millis(346);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, true);

        assert!(timeline.update(expected, false).is_empty());
        assert_eq!(
            timeline.update_with_detected_at(confirmed_at, true, Some(first_match)),
            [TimelineAction::Triggered]
        );
        assert_eq!(
            timeline.expected_at(),
            first_match.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn observed_jitter_does_not_change_the_configured_period() {
        let first_trigger = Instant::now();
        let second_trigger = first_trigger + Duration::from_millis(20_180);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update_with_detected_at(first_trigger, true, Some(first_trigger));

        timeline.update_with_detected_at(
            second_trigger + Duration::from_millis(166),
            true,
            Some(second_trigger),
        );
        assert_eq!(
            timeline.expected_at(),
            second_trigger.checked_add(Duration::from_secs(20))
        );
    }
}
