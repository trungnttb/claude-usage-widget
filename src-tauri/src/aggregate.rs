//! Folds raw usage records into the shape the widget renders.

use std::collections::HashMap;

use chrono::{DateTime, Local, NaiveDate, TimeZone};

use crate::snapshot::{DayTotals, ModelTotals, ProjectTotals, SessionTotals, TokenTotals};
use crate::transcript::UsageRecord;

/// Days of history the bar chart shows.
pub const CHART_DAYS: i64 = 14;

/// A session with no activity for this long is no longer "current".
const SESSION_IDLE_SECONDS: i64 = 30 * 60;

#[derive(Debug, Default)]
pub struct Aggregates {
    pub today: DayTotals,
    /// `CHART_DAYS` entries ending today, oldest first, including empty days.
    pub days: Vec<DayTotals>,
    pub by_model: Vec<ModelTotals>,
    pub by_project: Vec<ProjectTotals>,
    pub current_session: Option<SessionTotals>,
    pub has_estimated_pricing: bool,
    pub avg_7d_cost: f64,
    pub today_vs_avg: Option<f64>,
    pub cache_hit_rate: f64,
}

/// Days behind today that the average covers.
const AVERAGE_DAYS: usize = 7;

/// Mean cost of the `AVERAGE_DAYS` days before today.
///
/// Today is excluded because it is still in progress, and comparing it against
/// an average that already contains it would flatten exactly the spike the
/// comparison exists to show. Days with no usage are skipped rather than
/// counted as zero: a weekend off should not halve the weekday baseline.
fn average_cost(days: &[DayTotals]) -> f64 {
    let before_today = &days[..days.len().saturating_sub(1)];
    let window = &before_today[before_today.len().saturating_sub(AVERAGE_DAYS)..];

    let active: Vec<f64> = window
        .iter()
        .map(|day| day.cost_usd)
        .filter(|cost| *cost > 0.0)
        .collect();

    if active.is_empty() {
        return 0.0;
    }
    active.iter().sum::<f64>() / active.len() as f64
}

/// Share of input volume served from cache, for today.
///
/// A drop here means something is invalidating the prompt prefix, which costs
/// real money: a cache read bills at a tenth of the input rate, a miss at full
/// rate plus the write.
fn cache_hit_rate(today: &TokenTotals) -> f64 {
    let total = today.cache_read + today.cache_write + today.input;
    if total == 0 {
        return 0.0;
    }
    today.cache_read as f64 / total as f64
}

/// `now` is unix seconds, taken as an argument so the result is testable.
pub fn aggregate(records: &[UsageRecord], now: i64) -> Aggregates {
    let today = local_date(now);
    let mut result = Aggregates {
        days: empty_days(today),
        ..Default::default()
    };

    let mut by_date: HashMap<NaiveDate, DayTotals> = HashMap::new();
    let mut by_model: HashMap<&str, ModelTotals> = HashMap::new();
    let mut by_project: HashMap<&str, f64> = HashMap::new();

    for record in records {
        if !record.exact_pricing {
            result.has_estimated_pricing = true;
        }

        // Records whose timestamp could not be parsed still count towards
        // lifetime figures elsewhere, but they cannot be placed on a day.
        if record.timestamp == 0 {
            continue;
        }
        let date = local_date(record.timestamp);

        let day = by_date.entry(date).or_insert_with(|| DayTotals {
            date: date.to_string(),
            ..Default::default()
        });
        day.cost_usd += record.cost_usd;
        add_tokens(&mut day.tokens, record);

        if date != today {
            continue;
        }

        let model = by_model
            .entry(record.model.as_str())
            .or_insert_with(|| ModelTotals {
                model: record.model.clone(),
                ..Default::default()
            });
        model.cost_usd += record.cost_usd;
        add_tokens(&mut model.tokens, record);

        *by_project.entry(record.project.as_str()).or_insert(0.0) += record.cost_usd;
    }

    for day in &mut result.days {
        if let Some(totals) = by_date.get(&parse_date(&day.date)) {
            *day = totals.clone();
        }
    }
    result.today = by_date.get(&today).cloned().unwrap_or_else(|| DayTotals {
        date: today.to_string(),
        ..Default::default()
    });

    result.by_model = sorted_by_cost(by_model.into_values().collect(), |m| m.cost_usd);
    result.by_project = sorted_by_cost(
        by_project
            .into_iter()
            .map(|(project, cost_usd)| ProjectTotals {
                project: project.to_string(),
                cost_usd,
            })
            .collect(),
        |p| p.cost_usd,
    );
    result.current_session = current_session(records, now);
    result.avg_7d_cost = average_cost(&result.days);
    result.today_vs_avg =
        (result.avg_7d_cost > 0.0).then(|| result.today.cost_usd / result.avg_7d_cost);
    result.cache_hit_rate = cache_hit_rate(&result.today.tokens);

    result
}

fn sorted_by_cost<T>(mut items: Vec<T>, cost: impl Fn(&T) -> f64) -> Vec<T> {
    items.sort_by(|a, b| cost(b).total_cmp(&cost(a)));
    items
}

/// The session holding the most recent record, if it is still active.
fn current_session(records: &[UsageRecord], now: i64) -> Option<SessionTotals> {
    let latest = records
        .iter()
        .filter(|record| !record.session_id.is_empty())
        .max_by_key(|record| record.timestamp)?;

    if now - latest.timestamp > SESSION_IDLE_SECONDS {
        return None;
    }

    let mut totals = SessionTotals {
        session_id: latest.session_id.clone(),
        project: latest.project.clone(),
        started_at: latest.timestamp,
        ..Default::default()
    };
    for record in records
        .iter()
        .filter(|record| record.session_id == latest.session_id)
    {
        totals.cost_usd += record.cost_usd;
        add_tokens(&mut totals.tokens, record);
        totals.started_at = totals.started_at.min(record.timestamp);
    }
    Some(totals)
}

fn add_tokens(target: &mut TokenTotals, record: &UsageRecord) {
    target.input += record.tokens.input;
    target.output += record.tokens.output;
    target.cache_read += record.tokens.cache_read;
    target.cache_write += record.tokens.cache_write_5m + record.tokens.cache_write_1h;
}

/// Days are the user's days, so grouping follows the machine's timezone rather
/// than the UTC the transcript timestamps are written in.
fn local_date(unix_seconds: i64) -> NaiveDate {
    Local
        .timestamp_opt(unix_seconds, 0)
        .single()
        .map(|dt| dt.date_naive())
        .unwrap_or_else(|| {
            DateTime::from_timestamp(unix_seconds, 0)
                .map(|dt| dt.date_naive())
                .unwrap_or_default()
        })
}

fn parse_date(text: &str) -> NaiveDate {
    text.parse().unwrap_or_default()
}

fn empty_days(today: NaiveDate) -> Vec<DayTotals> {
    (0..CHART_DAYS)
        .rev()
        .map(|back| {
            let date = today - chrono::Duration::days(back);
            DayTotals {
                date: date.to_string(),
                ..Default::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::TokenCounts;

    fn at(days_ago: i64, cost: f64) -> UsageRecord {
        record(days_ago, cost, "claude-opus-5", "proj", "sess")
    }

    fn record(days_ago: i64, cost: f64, model: &str, project: &str, session: &str) -> UsageRecord {
        let midday = Local::now().date_naive() - chrono::Duration::days(days_ago);
        let timestamp = Local
            .from_local_datetime(&midday.and_hms_opt(12, 0, 0).expect("noon"))
            .single()
            .expect("local noon")
            .timestamp();
        UsageRecord {
            timestamp,
            model: model.to_string(),
            tokens: TokenCounts {
                input: 10,
                output: 20,
                cache_read: 90,
                cache_write_5m: 5,
                cache_write_1h: 5,
            },
            cost_usd: cost,
            exact_pricing: true,
            project: project.to_string(),
            session_id: session.to_string(),
        }
    }

    fn now() -> i64 {
        Local::now().timestamp()
    }

    #[test]
    fn the_chart_always_has_fourteen_days_oldest_first() {
        let result = aggregate(&[], now());
        assert_eq!(result.days.len(), CHART_DAYS as usize);
        assert!(result.days[0].date < result.days[13].date);
        assert_eq!(result.days[13].date, Local::now().date_naive().to_string());
    }

    #[test]
    fn a_day_without_records_stays_in_the_chart_at_zero() {
        let result = aggregate(&[at(3, 5.0)], now());
        let zero_days = result.days.iter().filter(|d| d.cost_usd == 0.0).count();
        assert_eq!(zero_days, 13);
        assert_eq!(result.days[CHART_DAYS as usize - 4].cost_usd, 5.0);
    }

    #[test]
    fn today_sums_only_todays_records() {
        let result = aggregate(&[at(0, 2.0), at(0, 3.0), at(1, 99.0)], now());
        assert_eq!(result.today.cost_usd, 5.0);
        assert_eq!(result.today.tokens.output, 40);
    }

    #[test]
    fn cache_write_tiers_are_summed_for_display() {
        let result = aggregate(&[at(0, 1.0)], now());
        assert_eq!(result.today.tokens.cache_write, 10);
    }

    #[test]
    fn models_and_projects_cover_today_only_and_sort_by_cost() {
        let records = vec![
            record(0, 1.0, "claude-sonnet-5", "small", "s"),
            record(0, 9.0, "claude-opus-5", "big", "s"),
            record(2, 50.0, "claude-fable-5", "yesterday", "s"),
        ];
        let result = aggregate(&records, now());

        assert_eq!(result.by_model.len(), 2);
        assert_eq!(result.by_model[0].model, "claude-opus-5");
        assert_eq!(result.by_project[0].project, "big");
        assert_eq!(result.by_project[1].project, "small");
    }

    /// Whether a session counts as current is measured against `now`, so these
    /// records are placed relative to it rather than at a fixed hour — a
    /// midday timestamp is only "recent" if the test happens to run at midday.
    fn seconds_ago(seconds: i64, cost: f64, session: &str) -> UsageRecord {
        UsageRecord {
            timestamp: now() - seconds,
            session_id: session.to_string(),
            cost_usd: cost,
            ..at(0, cost)
        }
    }

    #[test]
    fn the_current_session_covers_all_its_days_not_just_today() {
        let records = vec![
            seconds_ago(30 * 3600, 4.0, "long"),
            seconds_ago(60, 6.0, "long"),
        ];
        let session = aggregate(&records, now()).current_session.expect("session");
        assert_eq!(session.session_id, "long");
        assert_eq!(session.cost_usd, 10.0);
        assert!(session.started_at < now() - 3600);
    }

    #[test]
    fn an_idle_session_is_not_current() {
        let mut record = at(0, 1.0);
        record.timestamp = now() - SESSION_IDLE_SECONDS - 1;
        assert!(aggregate(&[record], now()).current_session.is_none());
    }

    #[test]
    fn a_record_priced_by_fallback_flags_the_whole_snapshot() {
        let mut record = at(0, 1.0);
        record.exact_pricing = false;
        assert!(aggregate(&[record], now()).has_estimated_pricing);
        assert!(!aggregate(&[at(0, 1.0)], now()).has_estimated_pricing);
    }

    #[test]
    fn the_average_skips_days_with_no_usage() {
        // Two active days out of the seven before today.
        let result = aggregate(&[at(1, 10.0), at(3, 20.0)], now());
        assert_eq!(result.avg_7d_cost, 15.0, "not 30/7");
    }

    #[test]
    fn the_average_excludes_today() {
        let result = aggregate(&[at(0, 1000.0), at(1, 10.0)], now());
        assert_eq!(result.avg_7d_cost, 10.0);
        assert_eq!(result.today_vs_avg, Some(100.0));
    }

    #[test]
    fn the_average_ignores_days_beyond_the_window() {
        let result = aggregate(&[at(1, 10.0), at(10, 500.0)], now());
        assert_eq!(result.avg_7d_cost, 10.0);
    }

    #[test]
    fn no_history_yields_no_comparison_rather_than_a_broken_number() {
        let result = aggregate(&[at(0, 5.0)], now());
        assert_eq!(result.avg_7d_cost, 0.0);
        assert_eq!(result.today_vs_avg, None);
    }

    #[test]
    fn cache_hit_rate_is_zero_rather_than_not_a_number_when_idle() {
        let result = aggregate(&[], now());
        assert_eq!(result.cache_hit_rate, 0.0);
        assert!(result.cache_hit_rate.is_finite());
    }

    #[test]
    fn cache_hit_rate_counts_reads_against_all_input_volume() {
        // Each record carries input 10, cache_read 90, cache_write 10.
        let result = aggregate(&[at(0, 1.0)], now());
        assert!((result.cache_hit_rate - 0.818_181_8).abs() < 1e-6);
    }

    #[test]
    fn a_record_without_a_timestamp_is_left_off_the_chart() {
        let mut record = at(0, 7.0);
        record.timestamp = 0;
        let result = aggregate(&[record], now());
        assert_eq!(result.today.cost_usd, 0.0);
    }
}
