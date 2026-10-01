//! Runs the two sources on their own cadences and assembles the snapshot.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use chrono::Local;

use crate::aggregate;
use crate::forecast;
use crate::settings::Settings;
use crate::snapshot::Snapshot;
use crate::transcript::Scanner;
use crate::usage_cli::LimitsSource;

/// How much history is kept in memory. Wider than the chart so the seven-day
/// average survives a run that spans midnight.
const RETAIN_DAYS: i64 = 32;

/// Holds both sources. They are locked separately so a `/usage` poll, which
/// takes about three seconds, never blocks a transcript refresh.
pub struct Collector {
    pub transcripts: Mutex<Scanner>,
    pub limits: Mutex<LimitsSource>,
}

impl Collector {
    pub fn new(settings: &Settings) -> Self {
        let root = crate::transcript::default_projects_dir().unwrap_or_default();
        let mut limits = LimitsSource::new(Duration::from_secs(settings.usage_interval_seconds));
        limits.set_claude_path(settings.claude_path.as_deref().map(PathBuf::from));
        Self {
            transcripts: Mutex::new(Scanner::new(root)),
            limits: Mutex::new(limits),
        }
    }

    /// Reads whatever has been appended since the last pass.
    pub fn refresh_transcripts(&self, now: i64) {
        let mut scanner = self.transcripts.lock().expect("scanner lock");
        scanner.scan();
        scanner.prune_before(now - RETAIN_DAYS * 24 * 60 * 60);
    }

    /// Runs the usage command. Returns how long to wait before the next call,
    /// which widens while the command keeps failing.
    pub fn refresh_limits(&self, now: i64) -> Duration {
        let mut limits = self.limits.lock().expect("limits lock");
        limits.refresh(now);
        limits.next_interval()
    }

    /// Builds the value the frontend renders.
    ///
    /// Each source is read under its own short lock and released before the
    /// next, so assembling a snapshot cannot stall either poll.
    pub fn snapshot(&self, settings: &Settings, now: i64) -> Snapshot {
        let aggregates = {
            let scanner = self.transcripts.lock().expect("scanner lock");
            aggregate::aggregate(scanner.records(), now)
        };

        let (limits, limits_read_at, limits_error) = {
            let source = self.limits.lock().expect("limits lock");
            if settings.usage_enabled {
                (
                    source.limits().cloned(),
                    source.read_at(),
                    source.error().cloned(),
                )
            } else {
                // Turned off, so there is nothing to report and nothing broken.
                (None, None, None)
            }
        };

        let limits = limits.map(|mut limits| {
            forecast::apply(&mut limits, now);
            limits
        });

        Snapshot {
            limits,
            limits_read_at,
            limits_error,
            today: aggregates.today,
            days: aggregates.days,
            avg_7d_cost: aggregates.avg_7d_cost,
            today_vs_avg: aggregates.today_vs_avg,
            by_model: aggregates.by_model,
            by_project: aggregates.by_project,
            current_session: aggregates.current_session,
            cache_hit_rate: aggregates.cache_hit_rate,
            scanned_at: now,
            has_estimated_pricing: aggregates.has_estimated_pricing,
        }
    }
}

pub fn now() -> i64 {
    Local::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage_cli::UsageError;

    const REPORT: &str = "Current session: 41% used · resets Sep 10, 2:40pm (Asia/Ho_Chi_Minh)";

    fn collector() -> Collector {
        Collector::new(&Settings::default())
    }

    #[test]
    fn a_snapshot_without_either_source_is_still_well_formed() {
        let snapshot = collector().snapshot(&Settings::default(), now());
        assert_eq!(snapshot.days.len(), aggregate::CHART_DAYS as usize);
        assert!(snapshot.limits.is_none());
        assert!(snapshot.cache_hit_rate.is_finite());
    }

    #[test]
    fn limits_carry_a_forecast_once_they_are_read() {
        let collector = collector();
        // A window half spent, half gone.
        let now = now();
        let report = format!(
            "Current session: 50% used · resets {} (Local)",
            reset_text(now + forecast::SESSION_WINDOW / 2)
        );
        collector.limits.lock().unwrap().apply(Ok(report), now);

        let snapshot = collector.snapshot(&Settings::default(), now);
        let session = snapshot.limits.expect("limits").session.expect("session");
        assert!(session.forecast.is_some());
    }

    /// Turning the source off must hide it rather than show it as broken.
    #[test]
    fn a_disabled_usage_source_reports_neither_limits_nor_an_error() {
        let collector = collector();
        collector
            .limits
            .lock()
            .unwrap()
            .apply(Err(UsageError::NotFound), now());

        let settings = Settings {
            usage_enabled: false,
            ..Default::default()
        };
        let snapshot = collector.snapshot(&settings, now());
        assert!(snapshot.limits.is_none());
        assert!(snapshot.limits_error.is_none());
    }

    /// A broken usage command must not stop the cost figures.
    #[test]
    fn a_failing_usage_source_leaves_the_transcript_figures_intact() {
        let collector = collector();
        collector
            .limits
            .lock()
            .unwrap()
            .apply(Err(UsageError::NotFound), now());

        let snapshot = collector.snapshot(&Settings::default(), now());
        assert!(snapshot.limits_error.is_some());
        assert_eq!(snapshot.days.len(), aggregate::CHART_DAYS as usize);
    }

    #[test]
    fn a_stale_reading_is_stamped_with_when_it_was_taken() {
        let collector = collector();
        let taken_at = now() - 600;
        collector
            .limits
            .lock()
            .unwrap()
            .apply(Ok(REPORT.to_string()), taken_at);

        let snapshot = collector.snapshot(&Settings::default(), now());
        assert_eq!(snapshot.limits_read_at, Some(taken_at));
    }

    fn reset_text(timestamp: i64) -> String {
        use chrono::TimeZone;
        Local
            .timestamp_opt(timestamp, 0)
            .single()
            .expect("local")
            .format("%b %-d, %-I:%M%P")
            .to_string()
    }
}
