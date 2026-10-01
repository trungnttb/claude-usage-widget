//! The single struct the frontend renders. Changing anything here means
//! changing the frontend in the same commit.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// None until `/usage` has been read at least once.
    pub limits: Option<Limits>,
    /// Unix seconds of the last successful `/usage` read, so the UI can mark a
    /// stale reading instead of showing it as current.
    pub limits_read_at: Option<i64>,
    pub limits_error: Option<LimitsError>,

    pub today: DayTotals,
    /// Most recent 14 days, oldest first.
    pub days: Vec<DayTotals>,
    /// Mean cost of the seven days before today, ignoring days with no usage.
    pub avg_7d_cost: f64,
    /// Today's cost as a multiple of `avg_7d_cost`. None when there is no
    /// average to compare against.
    pub today_vs_avg: Option<f64>,

    pub by_model: Vec<ModelTotals>,
    pub by_project: Vec<ProjectTotals>,
    pub current_session: Option<SessionTotals>,
    pub cache_hit_rate: f64,

    pub scanned_at: i64,
    /// True when at least one record used a model absent from the price table.
    pub has_estimated_pricing: bool,
}

/// Why the limits could not be read.
///
/// A code rather than a sentence: the wording belongs to whichever language
/// the user picked, and that choice lives in the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LimitsErrorCode {
    /// The command ran and reported success, but printed a cost summary in
    /// place of the limits — nobody is signed in, or there is no subscription.
    NotLoggedIn,
    /// It ran, but no line matched. Most likely the report was reworded.
    ReportChanged,
    CommandNotFound,
    TimedOut,
    CommandFailed,
    UnreadableOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitsError {
    pub code: LimitsErrorCode,
    /// Raw text from the command, shown under the message as supporting
    /// evidence. Never translated — it is quoting another program.
    pub detail: Option<String>,
}

impl LimitsError {
    pub fn new(code: LimitsErrorCode) -> Self {
        Self { code, detail: None }
    }

    pub fn with_detail(code: LimitsErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: Some(detail.into()),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub session: Option<Window>,
    pub week_all: Option<Window>,
    /// Per-model weekly limit. The model name varies, so it is carried as text.
    pub week_model: Option<NamedWindow>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub percent_used: f64,
    /// Unix seconds.
    pub resets_at: i64,
    pub forecast: Option<Forecast>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedWindow {
    pub model_name: String,
    #[serde(flatten)]
    pub window: Window,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Forecast {
    /// Percent the current rate reaches by reset time.
    pub projected_percent: f64,
    /// Unix seconds at which 100% is reached, when that happens before reset.
    pub exhausted_at: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayTotals {
    /// Local date as YYYY-MM-DD.
    pub date: String,
    pub cost_usd: f64,
    pub tokens: TokenTotals,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenTotals {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTotals {
    pub model: String,
    pub cost_usd: f64,
    pub tokens: TokenTotals,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTotals {
    pub project: String,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTotals {
    pub session_id: String,
    pub project: String,
    pub cost_usd: f64,
    pub tokens: TokenTotals,
    pub started_at: i64,
}
