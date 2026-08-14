use std::time::{Duration, Instant};

use super::model::DEFAULT_DEADLINE_GRACE_MS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelinePhase {
    Stopped,
    Waiting,
    Tracking,
    Prewarning,
    Confirming,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineAction {
    Triggered,
    PrewarnThree,
    PrewarnTwo,
    PrewarnOne,
    ConfirmationPending,
    Reset,
}

pub struct BuffTimeline {
    phase: TimelinePhase,
    cycle: Duration,
    deadline_confirmation_grace: Duration,
    expected_at: Option<Instant>,
    armed: bool,
    absence_observed: bool,
    sent_three: bool,
    sent_two: bool,
    sent_one: bool,
}

impl BuffTimeline {
    pub fn new(cycle_ms: u64) -> Self {
        Self {
            phase: TimelinePhase::Stopped,
            cycle: Duration::from_millis(cycle_ms),
            deadline_confirmation_grace: Duration::from_millis(DEFAULT_DEADLINE_GRACE_MS),
            expected_at: None,
            armed: false,
            absence_observed: false,
            sent_three: false,
            sent_two: false,
            sent_one: false,
        }
    }

    pub fn start_waiting(&mut self, cycle_ms: u64) {
        self.cycle = Duration::from_millis(cycle_ms);
        self.phase = TimelinePhase::Waiting;
        self.expected_at = None;
        self.armed = false;
        self.absence_observed = false;
        self.sent_three = false;
        self.sent_two = false;
        self.sent_one = false;
    }

    pub fn start_waiting_with_grace(&mut self, cycle_ms: u64, deadline_grace_ms: u64) {
        self.start_waiting(cycle_ms);
        self.deadline_confirmation_grace = Duration::from_millis(deadline_grace_ms);
    }

    pub fn stop(&mut self) {
        self.phase = TimelinePhase::Stopped;
        self.expected_at = None;
        self.armed = false;
        self.absence_observed = false;
        self.sent_three = false;
        self.sent_two = false;
        self.sent_one = false;
    }

    pub fn reset_waiting(&mut self) {
        self.phase = TimelinePhase::Waiting;
        self.expected_at = None;
        self.armed = false;
        self.absence_observed = false;
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
        self.update_with_detected_at(now, icon_present, !icon_present, None)
    }

    pub fn update_with_detected_at(
        &mut self,
        now: Instant,
        icon_present: bool,
        absence_confirmed: bool,
        detected_at: Option<Instant>,
    ) -> Vec<TimelineAction> {
        match self.phase {
            TimelinePhase::Stopped => Vec::new(),
            TimelinePhase::Waiting => {
                if !self.armed {
                    self.armed = absence_confirmed;
                    return Vec::new();
                }
                if icon_present {
                    self.anchor(valid_detection_time(now, detected_at));
                    vec![TimelineAction::Triggered]
                } else {
                    Vec::new()
                }
            }
            TimelinePhase::Tracking | TimelinePhase::Prewarning | TimelinePhase::Confirming => {
                self.update_anchored(now, icon_present, absence_confirmed, detected_at)
            }
        }
    }

    fn update_anchored(
        &mut self,
        now: Instant,
        icon_present: bool,
        absence_confirmed: bool,
        detected_at: Option<Instant>,
    ) -> Vec<TimelineAction> {
        self.absence_observed |= absence_confirmed;
        let Some(expected_at) = self.expected_at else {
            self.reset_waiting();
            return vec![TimelineAction::Reset];
        };

        if now >= expected_at {
            let grace_deadline = expected_at + self.deadline_confirmation_grace;
            let detected_within_grace =
                detected_at.is_some_and(|detected| detected <= grace_deadline);
            if icon_present && (now <= grace_deadline || detected_within_grace) {
                self.anchor(valid_detection_time(now, detected_at));
                return vec![TimelineAction::Triggered];
            }
            if now < grace_deadline {
                if self.phase != TimelinePhase::Confirming {
                    self.phase = TimelinePhase::Confirming;
                    return vec![TimelineAction::ConfirmationPending];
                }
                return Vec::new();
            }
            if icon_present && self.absence_observed {
                self.anchor(valid_detection_time(now, detected_at));
                return vec![TimelineAction::Triggered];
            }
            self.reset_waiting_after_missed_deadline();
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
        self.armed = true;
        self.absence_observed = false;
        self.expected_at = now.checked_add(self.cycle);
        self.sent_three = false;
        self.sent_two = false;
        self.sent_one = false;
    }

    fn reset_waiting_after_missed_deadline(&mut self) {
        let armed = self.absence_observed;
        self.reset_waiting();
        self.armed = armed;
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
    fn ignores_an_icon_that_was_already_present_when_monitoring_started() {
        let start = Instant::now();
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);

        assert!(
            timeline
                .update_with_detected_at(start, true, false, Some(start))
                .is_empty()
        );
        assert!(
            timeline
                .update_with_detected_at(start + Duration::from_secs(2), false, false, None,)
                .is_empty()
        );
        assert!(
            timeline
                .update_with_detected_at(start + Duration::from_secs(3), false, true, None,)
                .is_empty()
        );
        assert_eq!(timeline.phase(), TimelinePhase::Waiting);
        assert_eq!(timeline.expected_at(), None);

        let next_trigger = start + Duration::from_secs(8);
        assert_eq!(
            timeline.update_with_detected_at(next_trigger, true, false, Some(next_trigger),),
            [TimelineAction::Triggered]
        );
    }

    #[test]
    fn ignores_icon_presence_before_the_twenty_second_deadline() {
        let start = Instant::now();
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, false);
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
    fn emits_prealerts_once_before_the_deadline() {
        let start = Instant::now();
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, false);
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
        timeline.update(start, false);
        timeline.update(start, true);
        assert_eq!(
            timeline.update(start + Duration::from_secs(20), false),
            [TimelineAction::ConfirmationPending]
        );
        assert_eq!(timeline.phase(), TimelinePhase::Confirming);
        assert!(
            timeline
                .update(start + Duration::from_millis(20_300), false)
                .is_empty()
        );
        let actions = timeline.update(start + Duration::from_millis(21_500), false);
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
        timeline.update(start, false);
        timeline.update(start, true);
        assert_eq!(
            timeline.update(start + Duration::from_millis(21_500), false),
            [TimelineAction::Reset]
        );

        let detected_again = start + Duration::from_secs(24);
        timeline.update(start + Duration::from_secs(21), false);
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
        timeline.update(first_match, false);

        assert_eq!(
            timeline.update_with_detected_at(confirmed_at, true, false, Some(first_match)),
            [TimelineAction::Triggered]
        );
        assert_eq!(
            timeline.expected_at(),
            first_match.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn next_trigger_within_grace_reanchors_at_the_first_matching_frame() {
        let start = Instant::now();
        let expected = start + Duration::from_secs(20);
        let first_match = expected - Duration::from_millis(100);
        let confirmed_at = expected + Duration::from_millis(70);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, false);
        timeline.update(start, true);

        assert_eq!(
            timeline.update_with_detected_at(confirmed_at, true, false, Some(first_match)),
            [TimelineAction::Triggered]
        );
        assert_eq!(
            timeline.expected_at(),
            first_match.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn invalid_future_detection_timestamp_falls_back_to_confirmation_time() {
        let start = Instant::now();
        let confirmed_at = start + Duration::from_millis(20_100);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, false);
        timeline.update(start, true);

        timeline.update_with_detected_at(
            confirmed_at,
            true,
            false,
            Some(confirmed_at + Duration::from_secs(1)),
        );
        assert_eq!(
            timeline.expected_at(),
            confirmed_at.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn trigger_confirmed_at_1500_ms_is_accepted_and_reanchored() {
        let start = Instant::now();
        let expected = start + Duration::from_secs(20);
        let first_match = expected + Duration::from_millis(433);
        let confirmed_at = expected + Duration::from_millis(1_500);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, false);
        timeline.update(start, true);

        assert_eq!(
            timeline.update(expected, false),
            [TimelineAction::ConfirmationPending]
        );
        assert_eq!(
            timeline.update_with_detected_at(confirmed_at, true, false, Some(first_match)),
            [TimelineAction::Triggered]
        );
        assert_eq!(
            timeline.expected_at(),
            first_match.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn trigger_started_within_grace_is_accepted_when_confirmed_after_grace() {
        let start = Instant::now();
        let expected = start + Duration::from_secs(20);
        let confirmed_at = expected + Duration::from_millis(1_501);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, false);
        timeline.update(start, true);

        assert_eq!(
            timeline.update_with_detected_at(confirmed_at, true, false, Some(expected)),
            [TimelineAction::Triggered]
        );
        assert_eq!(timeline.phase(), TimelinePhase::Tracking);
        assert_eq!(
            timeline.expected_at(),
            expected.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn confirmed_trigger_after_grace_starts_a_fresh_timeline() {
        let start = Instant::now();
        let expected = start + Duration::from_secs(20);
        let detected_at = expected + Duration::from_secs(2);
        let confirmed_at = detected_at + Duration::from_millis(166);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, false);
        timeline.update(start, true);
        timeline.update(expected - Duration::from_secs(1), false);

        assert_eq!(
            timeline.update_with_detected_at(confirmed_at, true, false, Some(detected_at)),
            [TimelineAction::Triggered]
        );
        assert_eq!(
            timeline.expected_at(),
            detected_at.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn missed_deadline_keeps_confirmed_absence_armed_for_the_next_trigger() {
        let start = Instant::now();
        let expected = start + Duration::from_secs(20);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(start, false);
        timeline.update(start, true);
        timeline.update(expected - Duration::from_secs(1), false);

        assert_eq!(
            timeline.update_with_detected_at(
                expected + Duration::from_millis(1_501),
                false,
                false,
                None,
            ),
            [TimelineAction::Reset]
        );

        let detected_at = expected + Duration::from_secs(2);
        assert_eq!(
            timeline.update_with_detected_at(
                detected_at + Duration::from_millis(166),
                true,
                false,
                Some(detected_at),
            ),
            [TimelineAction::Triggered]
        );
    }

    #[test]
    fn configured_deadline_grace_controls_when_prediction_resets() {
        let start = Instant::now();
        let expected = start + Duration::from_secs(20);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting_with_grace(20_000, 300);
        timeline.update(start, false);
        timeline.update(start, true);

        assert_eq!(
            timeline.update_with_detected_at(
                expected + Duration::from_millis(301),
                false,
                true,
                None,
            ),
            [TimelineAction::Reset]
        );
        assert_eq!(timeline.phase(), TimelinePhase::Waiting);
    }

    #[test]
    fn observed_trigger_time_reanchors_the_next_deadline() {
        let first_trigger = Instant::now();
        let expected = first_trigger + Duration::from_secs(20);
        let second_trigger = expected + Duration::from_millis(100);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting(20_000);
        timeline.update(first_trigger, false);
        timeline.update_with_detected_at(first_trigger, true, false, Some(first_trigger));

        timeline.update_with_detected_at(
            second_trigger + Duration::from_millis(166),
            true,
            false,
            Some(second_trigger),
        );
        assert_eq!(
            timeline.expected_at(),
            second_trigger.checked_add(Duration::from_secs(20))
        );
    }

    #[test]
    fn observed_longer_cycles_do_not_accumulate_prediction_error() {
        let first_trigger = Instant::now();
        let second_trigger = first_trigger + Duration::from_millis(20_200);
        let third_trigger = second_trigger + Duration::from_millis(20_300);
        let fourth_trigger = third_trigger + Duration::from_millis(20_300);
        let mut timeline = BuffTimeline::new(20_000);
        timeline.start_waiting_with_grace(20_000, 2_000);
        timeline.update(first_trigger, false);
        timeline.update_with_detected_at(first_trigger, true, false, Some(first_trigger));

        for trigger in [second_trigger, third_trigger, fourth_trigger] {
            let confirmed_at = trigger + Duration::from_millis(166);
            assert_eq!(
                timeline.update_with_detected_at(confirmed_at, true, false, Some(trigger)),
                [TimelineAction::Triggered]
            );
            assert_eq!(
                timeline.expected_at(),
                trigger.checked_add(Duration::from_secs(20))
            );
        }
    }
}
