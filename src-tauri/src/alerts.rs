//! Decides when a limit is worth interrupting the user for.

use std::collections::HashMap;

use crate::settings::Settings;
use crate::snapshot::{Snapshot, Window};

/// Which limit crossed its threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertWindow {
    Session,
    Week,
}

/// A crossing worth interrupting for, carrying values rather than sentences:
/// the wording is chosen later, in whichever language the user picked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Alert {
    pub window: AlertWindow,
    pub percent: f64,
    /// True when the current rate reaches the limit before the reset.
    pub will_run_out: bool,
}

/// Remembers which windows have already been announced.
///
/// Keyed by the window's reset time, so a fresh window is a fresh chance to
/// warn while the current one only ever warns once.
#[derive(Default)]
pub struct Alerts {
    announced: HashMap<&'static str, i64>,
    /// The first snapshot only records where things stand.
    primed: bool,
}

impl Alerts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns what to notify about, and records it as announced.
    pub fn check(&mut self, snapshot: &Snapshot, settings: &Settings) -> Vec<Alert> {
        let mut alerts = Vec::new();
        let Some(limits) = &snapshot.limits else {
            return alerts;
        };

        let candidates: [(&'static str, AlertWindow, Option<&Window>, f64); 2] = [
            (
                "session",
                AlertWindow::Session,
                limits.session.as_ref(),
                settings.session_alert_percent,
            ),
            (
                "week",
                AlertWindow::Week,
                limits.week_all.as_ref(),
                settings.week_alert_percent,
            ),
        ];

        for (key, which, window, threshold) in candidates {
            let Some(window) = window else { continue };
            if window.percent_used < threshold {
                // Back under the threshold, which only happens after a reset;
                // clearing here lets the next crossing be announced.
                self.announced.remove(key);
                continue;
            }
            if self.announced.get(key) == Some(&window.resets_at) {
                continue;
            }
            self.announced.insert(key, window.resets_at);

            // Opening the app when a limit is already passed should not fire a
            // notification for something that happened before it was running.
            if !self.primed || !settings.notifications_enabled {
                continue;
            }
            alerts.push(Alert {
                window: which,
                percent: window.percent_used,
                will_run_out: window
                    .forecast
                    .as_ref()
                    .is_some_and(|forecast| forecast.exhausted_at.is_some()),
            });
        }

        self.primed = true;
        alerts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::Limits;

    fn snapshot(session_percent: f64, resets_at: i64) -> Snapshot {
        Snapshot {
            limits: Some(Limits {
                session: Some(Window {
                    percent_used: session_percent,
                    resets_at,
                    forecast: None,
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn settings() -> Settings {
        Settings::default()
    }

    /// The first snapshot establishes where things stand. Firing on it would
    /// mean a notification every time the app starts while already over.
    #[test]
    fn the_first_snapshot_never_notifies() {
        let mut alerts = Alerts::new();
        assert!(alerts.check(&snapshot(95.0, 1000), &settings()).is_empty());
    }

    #[test]
    fn crossing_the_threshold_notifies_once() {
        let mut alerts = Alerts::new();
        alerts.check(&snapshot(10.0, 1000), &settings());

        assert_eq!(alerts.check(&snapshot(85.0, 1000), &settings()).len(), 1);
        assert!(alerts.check(&snapshot(86.0, 1000), &settings()).is_empty());
        assert!(alerts.check(&snapshot(99.0, 1000), &settings()).is_empty());
    }

    #[test]
    fn staying_below_the_threshold_notifies_nothing() {
        let mut alerts = Alerts::new();
        alerts.check(&snapshot(10.0, 1000), &settings());
        assert!(alerts.check(&snapshot(79.0, 1000), &settings()).is_empty());
    }

    /// A new window is a new chance to warn.
    #[test]
    fn the_next_window_can_notify_again() {
        let mut alerts = Alerts::new();
        alerts.check(&snapshot(10.0, 1000), &settings());
        assert_eq!(alerts.check(&snapshot(85.0, 1000), &settings()).len(), 1);

        // Reset: the window rolls over and usage starts again.
        alerts.check(&snapshot(5.0, 2000), &settings());
        assert_eq!(alerts.check(&snapshot(85.0, 2000), &settings()).len(), 1);
    }

    #[test]
    fn notifications_can_be_turned_off() {
        let quiet = Settings {
            notifications_enabled: false,
            ..Default::default()
        };
        let mut alerts = Alerts::new();
        alerts.check(&snapshot(10.0, 1000), &quiet);
        assert!(alerts.check(&snapshot(85.0, 1000), &quiet).is_empty());
    }

    #[test]
    fn a_custom_threshold_is_respected() {
        let eager = Settings {
            session_alert_percent: 40.0,
            ..Default::default()
        };
        let mut alerts = Alerts::new();
        alerts.check(&snapshot(10.0, 1000), &eager);
        assert_eq!(alerts.check(&snapshot(45.0, 1000), &eager).len(), 1);
    }

    #[test]
    fn no_limits_means_nothing_to_announce() {
        let mut alerts = Alerts::new();
        alerts.check(&snapshot(10.0, 1000), &settings());
        assert!(alerts.check(&Snapshot::default(), &settings()).is_empty());
    }
}
