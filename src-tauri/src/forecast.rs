//! Projects the current burn rate forward to the end of a limit window.
//!
//! The percentages say where usage stands; this says where it is heading, which
//! is the part a person can still act on.

use crate::snapshot::{Forecast, Limits, Window};

pub const SESSION_WINDOW: i64 = 5 * 60 * 60;
pub const WEEK_WINDOW: i64 = 7 * 24 * 60 * 60;

/// Below this much elapsed time the rate is dominated by whatever happened in
/// the first few minutes, and the projection swings wildly between polls.
const MIN_ELAPSED_SECONDS: i64 = 10 * 60;

/// Fills in the forecast on every window that has one.
pub fn apply(limits: &mut Limits, now: i64) {
    if let Some(window) = limits.session.as_mut() {
        window.forecast = forecast(window, SESSION_WINDOW, now);
    }
    if let Some(window) = limits.week_all.as_mut() {
        window.forecast = forecast(window, WEEK_WINDOW, now);
    }
    if let Some(named) = limits.week_model.as_mut() {
        named.window.forecast = forecast(&named.window, WEEK_WINDOW, now);
    }
}

/// `window_length` is how long the allowance runs for, in seconds. The window's
/// start is derived from its reset time rather than assumed, so a window that
/// began at an unusual hour still projects correctly.
pub fn forecast(window: &Window, window_length: i64, now: i64) -> Option<Forecast> {
    if window.resets_at == 0 || window.percent_used <= 0.0 {
        return None;
    }

    let started_at = window.resets_at - window_length;
    let elapsed = now - started_at;
    let remaining = window.resets_at - now;
    if elapsed < MIN_ELAPSED_SECONDS || remaining <= 0 {
        return None;
    }

    let rate_per_second = window.percent_used / elapsed as f64;
    let projected = window.percent_used + rate_per_second * remaining as f64;

    let exhausted_at = if projected >= 100.0 {
        let seconds_left = (100.0 - window.percent_used) / rate_per_second;
        Some(now + seconds_left.ceil() as i64)
    } else {
        None
    };

    Some(Forecast {
        projected_percent: projected,
        exhausted_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a window that resets `until_reset` seconds from `now`.
    fn window(percent: f64, until_reset: i64, now: i64) -> Window {
        Window {
            percent_used: percent,
            resets_at: now + until_reset,
            forecast: None,
        }
    }

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn a_steady_rate_projects_to_exactly_full_at_reset() {
        // Half the window gone, half the allowance used.
        let half = SESSION_WINDOW / 2;
        let result = forecast(&window(50.0, half, NOW), SESSION_WINDOW, NOW).expect("forecast");
        assert!((result.projected_percent - 100.0).abs() < 1e-9);
        assert!(result.exhausted_at.is_some());
    }

    #[test]
    fn a_slow_rate_lands_under_full_and_names_no_exhaustion_time() {
        let half = SESSION_WINDOW / 2;
        let result = forecast(&window(25.0, half, NOW), SESSION_WINDOW, NOW).expect("forecast");
        assert!((result.projected_percent - 50.0).abs() < 1e-9);
        assert_eq!(result.exhausted_at, None);
    }

    #[test]
    fn a_fast_rate_names_the_minute_it_runs_out() {
        // 80% used after 2 of 5 hours: 40%/hour, so the last 20% takes 30 min.
        let elapsed = 2 * 60 * 60;
        let until_reset = SESSION_WINDOW - elapsed;
        let result =
            forecast(&window(80.0, until_reset, NOW), SESSION_WINDOW, NOW).expect("forecast");

        let exhausted_at = result.exhausted_at.expect("exhausted");
        assert_eq!(exhausted_at, NOW + 30 * 60);
        assert!(
            exhausted_at < NOW + until_reset,
            "must land before the reset"
        );
    }

    /// Early in a window the rate is noise, so no projection is offered.
    #[test]
    fn the_first_minutes_of_a_window_produce_no_forecast() {
        let elapsed = 5 * 60;
        let until_reset = SESSION_WINDOW - elapsed;
        assert!(forecast(&window(3.0, until_reset, NOW), SESSION_WINDOW, NOW).is_none());
    }

    #[test]
    fn an_unused_window_produces_no_forecast() {
        let half = SESSION_WINDOW / 2;
        assert!(forecast(&window(0.0, half, NOW), SESSION_WINDOW, NOW).is_none());
    }

    #[test]
    fn a_window_with_an_unknown_reset_produces_no_forecast() {
        let mut w = window(50.0, 100, NOW);
        w.resets_at = 0;
        assert!(forecast(&w, SESSION_WINDOW, NOW).is_none());
    }

    #[test]
    fn a_window_already_past_its_reset_produces_no_forecast() {
        assert!(forecast(&window(50.0, -60, NOW), SESSION_WINDOW, NOW).is_none());
    }

    #[test]
    fn weekly_windows_are_measured_over_a_week() {
        // Half a week gone, 30% used, so it lands at 60%.
        let half = WEEK_WINDOW / 2;
        let result = forecast(&window(30.0, half, NOW), WEEK_WINDOW, NOW).expect("forecast");
        assert!((result.projected_percent - 60.0).abs() < 1e-9);
    }

    #[test]
    fn apply_fills_every_window_it_can() {
        let half_session = SESSION_WINDOW / 2;
        let half_week = WEEK_WINDOW / 2;
        let mut limits = Limits {
            session: Some(window(50.0, half_session, NOW)),
            week_all: Some(window(30.0, half_week, NOW)),
            week_model: Some(crate::snapshot::NamedWindow {
                model_name: "Fable".into(),
                window: window(0.0, half_week, NOW),
            }),
        };

        apply(&mut limits, NOW);

        assert!(limits.session.unwrap().forecast.is_some());
        assert!(limits.week_all.unwrap().forecast.is_some());
        assert!(
            limits.week_model.unwrap().window.forecast.is_none(),
            "an unused window still gets no forecast"
        );
    }
}
