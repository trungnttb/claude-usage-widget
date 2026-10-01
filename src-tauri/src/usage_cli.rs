//! Runs `claude -p "/usage"` and returns what it printed.
//!
//! This is the only way to read the subscription rate limits: the transcripts
//! on disk do not carry them. The command reports `total_cost_usd: 0`,
//! `num_turns: 0` and `duration_api_ms: 0`, so it spends no tokens — the cost
//! is the ~2.9s of CLI startup, which is why the caller runs it on a timer of
//! minutes rather than seconds.
//!
//! Finding the executable is its own problem on macOS. A bundled app is
//! started by launchd, not by a shell, so it inherits launchd's PATH rather
//! than the user's, and none of the directories the Claude Code installers
//! write to are on it. Windows GUI processes do inherit the user PATH, which
//! is why the bare name was enough there.

#[cfg(unix)]
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use chrono::{Datelike, Local, NaiveDate, NaiveTime, TimeZone};
use serde::Deserialize;

use crate::snapshot::{Limits, LimitsError, LimitsErrorCode, NamedWindow, Window};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the login-shell search may take before it is abandoned.
///
/// That search sources the user's rc files, which are free to do anything;
/// without a limit a slow one would stall the poll it is resolving for.
#[cfg(unix)]
const SHELL_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

const EXECUTABLE_NAME: &str = "claude";

/// Directory the command is run from, under the app's own config directory.
const WORKING_DIR_NAME: &str = "usage-cwd";

#[derive(Debug, Clone, PartialEq)]
pub enum UsageError {
    /// No `claude` executable on PATH.
    NotFound,
    TimedOut,
    /// Ran, but the output could not be used.
    Unusable(String),
}

impl UsageError {
    /// The code the widget words for the user, plus whatever the command said.
    pub fn as_limits_error(&self) -> LimitsError {
        match self {
            Self::NotFound => LimitsError::new(LimitsErrorCode::CommandNotFound),
            Self::TimedOut => LimitsError::new(LimitsErrorCode::TimedOut),
            Self::Unusable(detail) => {
                LimitsError::with_detail(LimitsErrorCode::UnreadableOutput, detail)
            }
        }
    }
}

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "claude command not found on PATH"),
            Self::TimedOut => write!(f, "claude did not answer in time"),
            Self::Unusable(detail) => write!(f, "{detail}"),
        }
    }
}

/// Runs the command and returns the human-readable usage report.
pub fn read_usage(program: &Path, timeout: Duration) -> Result<String, UsageError> {
    let output = run(program, timeout)?;
    extract_result(&output)
}

fn run(program: &Path, timeout: Duration) -> Result<String, UsageError> {
    let program = program.to_path_buf();
    with_timeout(timeout, move || spawn_and_wait(&program)).unwrap_or(Err(UsageError::TimedOut))
}

/// Runs `work` on its own thread and stops waiting for it after `timeout`.
///
/// A thread that ran out of time is left to finish: it owns the child process
/// handle, so reclaiming the child here would mean two waits on the same one.
/// The usage command normally finishes in about three seconds, so a timeout at
/// thirty means something is wrong upstream, which the caller backs off on.
fn with_timeout<T, F>(timeout: Duration, work: F) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    receiver.recv_timeout(timeout).ok()
}

fn spawn_and_wait(program: &Path) -> Result<String, UsageError> {
    let mut spawned = command(program);
    if let Some(dir) = working_dir() {
        spawned.current_dir(dir);
    }
    let output = spawned
        .args(["-p", "/usage", "--output-format", "json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => UsageError::NotFound,
            _ => UsageError::Unusable(error.to_string()),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(UsageError::Unusable(format!(
            "claude exited with {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(windows)]
fn command(program: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    /// Without this a console window flashes on screen at every poll, because
    /// a GUI process spawning a console application gets one allocated.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
fn command(program: &Path) -> Command {
    Command::new(program)
}

/// An empty directory of the app's own to run the command in.
///
/// The CLI takes its working directory to be the project it is looking at, and
/// a bundled app is launched with `/` as its working directory — from there the
/// CLI walks into every protected folder in the home directory, and macOS asks
/// the user to allow each one in the app's name. Returning None leaves the
/// inherited directory in place, which is wrong but still runs.
fn working_dir() -> Option<PathBuf> {
    let dir = working_dir_from(&crate::settings::default_path()?)?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn working_dir_from(settings_path: &Path) -> Option<PathBuf> {
    Some(settings_path.parent()?.join(WORKING_DIR_NAME))
}

/// Finds the executable, cheapest layer first: the inherited PATH, then the
/// directories the installers write to, then the user's login shell.
#[cfg(unix)]
fn resolve() -> Option<PathBuf> {
    search_inherited_path()
        .or_else(|| first_executable(&known_locations(home().as_deref())))
        .or_else(ask_login_shell)
}

/// A Windows GUI process inherits the user PATH, so the bare name resolves and
/// none of the search below is needed.
#[cfg(windows)]
fn resolve() -> Option<PathBuf> {
    Some(PathBuf::from(EXECUTABLE_NAME))
}

#[cfg(unix)]
fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(unix)]
fn search_inherited_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let candidates: Vec<PathBuf> = std::env::split_paths(&path)
        .map(|dir| dir.join(EXECUTABLE_NAME))
        .collect();
    first_executable(&candidates)
}

/// Where the installers put it, in the order they are tried.
#[cfg(unix)]
fn known_locations(home: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = home {
        // The native installer, then the layout `claude migrate-installer`
        // leaves behind, then bun.
        candidates.push(home.join(".local/bin").join(EXECUTABLE_NAME));
        candidates.push(home.join(".claude/local").join(EXECUTABLE_NAME));
        candidates.push(home.join(".bun/bin").join(EXECUTABLE_NAME));
        candidates.extend(nvm_locations(home));
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin").join(EXECUTABLE_NAME));
    candidates.push(PathBuf::from("/usr/local/bin").join(EXECUTABLE_NAME));
    candidates
}

/// `~/.nvm/versions/node/<version>/bin/claude`.
///
/// The directory name carries the Node version, so this path cannot be a
/// constant. Sorted by name so that a machine with several Node versions picks
/// the same one at every launch instead of whatever order the filesystem
/// happens to return.
#[cfg(unix)]
fn nvm_locations(home: &Path) -> Vec<PathBuf> {
    let mut versions: Vec<PathBuf> = std::fs::read_dir(home.join(".nvm/versions/node"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    versions.sort();
    versions
        .into_iter()
        .map(|dir| dir.join("bin").join(EXECUTABLE_NAME))
        .collect()
}

#[cfg(unix)]
fn first_executable(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|path| is_executable(path))
        .map(PathBuf::from)
}

/// A regular file carrying an execute bit. Directories carry those bits too,
/// and a directory named `claude` on PATH would otherwise be spawned.
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Asks the user's login shell where `claude` is.
///
/// `-i` as well as `-l`, because zsh reads `.zshrc` — where a PATH edit
/// usually goes — only for interactive shells. The output is filtered rather
/// than taken whole, since an rc file is free to print a banner of its own.
#[cfg(unix)]
fn ask_login_shell() -> Option<PathBuf> {
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| OsString::from("/bin/sh"));
    let output = with_timeout(SHELL_PROBE_TIMEOUT, move || {
        Command::new(shell)
            .args(["-lic", "command -v claude"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
    })?
    .ok()?;

    first_executable(&shell_output_paths(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

/// The absolute paths among the lines the shell printed.
#[cfg(unix)]
fn shell_output_paths(stdout: &str) -> Vec<PathBuf> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('/'))
        .map(PathBuf::from)
        .collect()
}

#[derive(Deserialize)]
struct CliEnvelope {
    result: Option<String>,
    #[serde(default)]
    is_error: bool,
}

/// Pulls the report text out of the CLI's JSON envelope.
fn extract_result(stdout: &str) -> Result<String, UsageError> {
    let envelope: CliEnvelope = serde_json::from_str(stdout.trim())
        .map_err(|error| UsageError::Unusable(format!("unreadable CLI output: {error}")))?;

    if envelope.is_error {
        return Err(UsageError::Unusable(
            envelope
                .result
                .unwrap_or_else(|| "claude reported an error".into()),
        ));
    }

    envelope
        .result
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| UsageError::Unusable("CLI output had no result field".into()))
}

/// Reads the limit lines out of the report.
///
/// The report is prose in English, so this is inherently brittle: Anthropic can
/// reword it at any time. Each line is therefore parsed independently and a
/// line that does not match yields None instead of failing the whole read, and
/// matching keys on `%`, `resets` and the bracketed name rather than on the
/// exact sentence.
pub fn parse_limits(text: &str, now: i64) -> Limits {
    let mut limits = Limits::default();

    for line in text.lines() {
        let line = line.trim();
        let Some(window) = parse_window(line, now) else {
            continue;
        };

        if line.starts_with("Current session") {
            limits.session = Some(window);
        } else if line.starts_with("Current week") {
            match bracketed_name(line) {
                Some(name) if name.eq_ignore_ascii_case("all models") => {
                    limits.week_all = Some(window)
                }
                // The per-model line names whichever model has its own weekly
                // allowance, so the name is carried rather than matched.
                Some(name) => {
                    limits.week_model = Some(NamedWindow {
                        model_name: name.to_string(),
                        window,
                    })
                }
                None => limits.week_all = Some(window),
            }
        }
    }

    limits
}

fn parse_window(line: &str, now: i64) -> Option<Window> {
    Some(Window {
        percent_used: parse_percent(line)?,
        resets_at: line
            .split_once("resets ")
            .and_then(|(_, rest)| parse_reset(rest, now))
            .unwrap_or(0),
        forecast: None,
    })
}

/// The number immediately before the first `%`.
fn parse_percent(line: &str) -> Option<f64> {
    let percent_at = line.find('%')?;
    let digits: String = line[..percent_at]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    digits.chars().rev().collect::<String>().parse().ok()
}

fn bracketed_name(line: &str) -> Option<&str> {
    let open = line.find('(')?;
    let close = line[open..].find(')')? + open;
    Some(line[open + 1..close].trim())
}

/// Parses `Sep 10, 2:40pm (Asia/Ho_Chi_Minh)` into unix seconds.
///
/// The report carries no year, so the candidate closest to now wins; that keeps
/// a reset on the far side of New Year from landing eleven months away. The
/// bracketed zone is the machine's own, since the CLI formats in local time, so
/// the timestamp is built as a local one and no timezone database is needed.
fn parse_reset(text: &str, now: i64) -> Option<i64> {
    let without_zone = text.split(" (").next()?.trim().trim_end_matches('.');
    let (date_part, time_part) = without_zone.split_once(", ")?;

    let (month_name, day_text) = date_part.trim().split_once(' ')?;
    let month = month_number(month_name)?;
    let day: u32 = day_text.trim().parse().ok()?;
    let (hour, minute) = parse_clock(time_part.trim())?;

    let current_year = Local.timestamp_opt(now, 0).single()?.year();
    [current_year - 1, current_year, current_year + 1]
        .into_iter()
        .filter_map(|year| {
            let date = NaiveDate::from_ymd_opt(year, month, day)?;
            let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
            Local
                .from_local_datetime(&date.and_time(time))
                .single()
                .map(|dt| dt.timestamp())
        })
        .min_by_key(|candidate| (candidate - now).abs())
}

/// Accepts `2:40pm` and the minute-less `4am` the report uses on the hour.
fn parse_clock(text: &str) -> Option<(u32, u32)> {
    let lower = text.to_ascii_lowercase();
    let (digits, is_pm) = match lower.strip_suffix("pm") {
        Some(rest) => (rest, true),
        None => (lower.strip_suffix("am")?, false),
    };

    let (hour_text, minute_text) = digits
        .trim()
        .split_once(':')
        .unwrap_or((digits.trim(), "0"));
    let hour: u32 = hour_text.parse().ok()?;
    let minute: u32 = minute_text.parse().ok()?;
    if hour == 0 || hour > 12 || minute > 59 {
        return None;
    }

    let hour = match (hour, is_pm) {
        (12, false) => 0,
        (12, true) => 12,
        (h, true) => h + 12,
        (h, false) => h,
    };
    Some((hour, minute))
}

fn month_number(name: &str) -> Option<u32> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let lower = name.to_ascii_lowercase();
    MONTHS
        .iter()
        .position(|month| lower.starts_with(month))
        .map(|index| index as u32 + 1)
}

/// Failures below this many are treated as noise and do not widen the interval.
const FAILURES_BEFORE_BACKOFF: u32 = 3;
const MAX_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Holds the limits between polls and decides when to poll again.
///
/// A poll costs ~3s of CLI startup, so repeating it on the normal cadence
/// while something is persistently broken wastes the machine to no purpose.
pub struct LimitsSource {
    limits: Option<Limits>,
    read_at: Option<i64>,
    error: Option<LimitsError>,
    consecutive_failures: u32,
    base_interval: Duration,
    /// An executable named in the settings, which wins over the search.
    claude_path: Option<PathBuf>,
    /// What the search settled on, kept so it runs once instead of at every
    /// poll — its last layer costs a shell startup.
    resolved: Option<PathBuf>,
}

impl LimitsSource {
    pub fn new(base_interval: Duration) -> Self {
        Self {
            limits: None,
            read_at: None,
            error: None,
            consecutive_failures: 0,
            base_interval,
            claude_path: None,
            resolved: None,
        }
    }

    pub fn set_interval(&mut self, interval: Duration) {
        self.base_interval = interval;
    }

    /// Points the source at a specific executable. `None` restores the search.
    pub fn set_claude_path(&mut self, path: Option<PathBuf>) {
        if self.claude_path != path {
            self.claude_path = path;
            self.resolved = None;
        }
    }

    /// The executable to spawn, searching for it on the first call only.
    fn program(&mut self) -> Result<PathBuf, UsageError> {
        if let Some(path) = self.claude_path.clone().or_else(|| self.resolved.clone()) {
            return Ok(path);
        }
        let found = resolve().ok_or(UsageError::NotFound)?;
        self.resolved = Some(found.clone());
        Ok(found)
    }

    pub fn limits(&self) -> Option<&Limits> {
        self.limits.as_ref()
    }

    /// Always Some when [`Self::limits`] is, so the widget can say how old the
    /// reading is instead of presenting a stale one as current.
    pub fn read_at(&self) -> Option<i64> {
        self.read_at
    }

    pub fn error(&self) -> Option<&LimitsError> {
        self.error.as_ref()
    }

    /// Runs the command and folds the outcome in.
    pub fn refresh(&mut self, now: i64) {
        let outcome = match self.program() {
            Ok(program) => read_usage(&program, DEFAULT_TIMEOUT),
            Err(error) => Err(error),
        };
        // An upgrade or an uninstall can move the executable. Searching again
        // on the next poll costs less than staying broken until a restart.
        if matches!(outcome, Err(UsageError::NotFound)) {
            self.resolved = None;
        }
        self.apply(outcome, now);
    }

    /// Split out from [`Self::refresh`] so the state machine is testable
    /// without spawning the CLI.
    pub fn apply(&mut self, outcome: Result<String, UsageError>, now: i64) {
        match outcome {
            Ok(text) => {
                let parsed = parse_limits(&text, now);
                if parsed.is_empty() {
                    // The command ran, so the install is fine; keeping the
                    // previous reading beats blanking the widget.
                    self.fail(explain_missing_limits(&text));
                    return;
                }
                self.limits = Some(parsed);
                self.read_at = Some(now);
                self.error = None;
                self.consecutive_failures = 0;
            }
            Err(error) => self.fail(error.as_limits_error()),
        }
    }

    /// A failed poll never clears what was read before it.
    fn fail(&mut self, error: LimitsError) {
        self.error = Some(error);
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
    }

    /// How long to wait before the next poll.
    pub fn next_interval(&self) -> Duration {
        if self.consecutive_failures < FAILURES_BEFORE_BACKOFF {
            return self.base_interval;
        }
        let doublings = self.consecutive_failures - FAILURES_BEFORE_BACKOFF + 1;
        let factor = 1u32.checked_shl(doublings).unwrap_or(u32::MAX);
        self.base_interval
            .saturating_mul(factor)
            .min(MAX_INTERVAL)
            .max(self.base_interval)
    }
}

impl Limits {
    /// True when no line could be read.
    pub fn is_empty(&self) -> bool {
        self.session.is_none() && self.week_all.is_none() && self.week_model.is_none()
    }
}

/// Says why a report carried no limits, in terms the person can act on.
///
/// Signing out is the common cause and the command does not announce it: it
/// exits zero with `is_error: false` and prints a cost summary in place of the
/// limit lines. Reporting that as a parse failure would send someone hunting
/// for a bug in the widget.
fn explain_missing_limits(report: &str) -> LimitsError {
    if report.contains("Total cost:") {
        return LimitsError::new(LimitsErrorCode::NotLoggedIn);
    }
    LimitsError::new(LimitsErrorCode::ReportChanged)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = "You are currently using your subscription to power your Claude Code usage

Current session: 41% used · resets Sep 10, 2:40pm (Asia/Ho_Chi_Minh)
Current week (all models): 23% used · resets Sep 15, 4am (Asia/Ho_Chi_Minh)
Current week (Fable): 22% used · resets Sep 15, 4am (Asia/Ho_Chi_Minh)

What's contributing to your limits usage?
Last 24h · 1115 requests · 11 sessions";

    fn sep_10_2026() -> i64 {
        Local
            .with_ymd_and_hms(2026, 9, 10, 12, 0, 0)
            .single()
            .expect("noon")
            .timestamp()
    }

    #[test]
    fn reads_all_three_lines() {
        let limits = parse_limits(REPORT, sep_10_2026());
        assert_eq!(limits.session.as_ref().expect("session").percent_used, 41.0);
        assert_eq!(limits.week_all.as_ref().expect("week").percent_used, 23.0);
        let model = limits.week_model.as_ref().expect("model week");
        assert_eq!(model.model_name, "Fable");
        assert_eq!(model.window.percent_used, 22.0);
    }

    #[test]
    fn the_per_model_line_is_not_matched_on_a_hard_coded_name() {
        let report = REPORT.replace("(Fable)", "(Some Future Model)");
        let limits = parse_limits(&report, sep_10_2026());
        assert_eq!(
            limits.week_model.expect("model week").model_name,
            "Some Future Model"
        );
    }

    #[test]
    fn reset_times_become_absolute() {
        let limits = parse_limits(REPORT, sep_10_2026());
        let expected = Local
            .with_ymd_and_hms(2026, 9, 10, 14, 40, 0)
            .single()
            .expect("2:40pm")
            .timestamp();
        assert_eq!(limits.session.expect("session").resets_at, expected);
    }

    #[test]
    fn an_hour_without_minutes_is_accepted() {
        let limits = parse_limits(REPORT, sep_10_2026());
        let expected = Local
            .with_ymd_and_hms(2026, 9, 15, 4, 0, 0)
            .single()
            .expect("4am")
            .timestamp();
        assert_eq!(limits.week_all.expect("week").resets_at, expected);
    }

    /// A reset just past New Year must not be read as eleven months away.
    #[test]
    fn the_year_is_inferred_from_proximity() {
        let new_years_eve = Local
            .with_ymd_and_hms(2026, 12, 31, 22, 0, 0)
            .single()
            .expect("nye")
            .timestamp();
        let line = "Current session: 50% used · resets Jan 1, 3am (Asia/Ho_Chi_Minh)";
        let resets_at = parse_limits(line, new_years_eve)
            .session
            .expect("session")
            .resets_at;
        let expected = Local
            .with_ymd_and_hms(2027, 1, 1, 3, 0, 0)
            .single()
            .expect("jan 1")
            .timestamp();
        assert_eq!(resets_at, expected);
    }

    #[test]
    fn midnight_and_noon_do_not_collide() {
        assert_eq!(parse_clock("12am"), Some((0, 0)));
        assert_eq!(parse_clock("12pm"), Some((12, 0)));
        assert_eq!(parse_clock("12:30am"), Some((0, 30)));
        assert_eq!(parse_clock("11:59pm"), Some((23, 59)));
    }

    #[test]
    fn nonsense_clocks_are_rejected() {
        assert_eq!(parse_clock("25pm"), None);
        assert_eq!(parse_clock("2:99pm"), None);
        assert_eq!(parse_clock("14:00"), None);
        assert_eq!(parse_clock(""), None);
    }

    #[test]
    fn a_reworded_report_yields_nothing_rather_than_wrong_numbers() {
        let limits = parse_limits("You have plenty of capacity left today.", sep_10_2026());
        assert!(limits.session.is_none());
        assert!(limits.week_all.is_none());
        assert!(limits.week_model.is_none());
    }

    /// One broken line must not take the other two with it.
    #[test]
    fn a_line_that_lost_its_reset_still_reports_its_percentage() {
        let report = "Current session: 41% used\nCurrent week (all models): 23% used · resets Sep 15, 4am (Asia/Ho_Chi_Minh)";
        let limits = parse_limits(report, sep_10_2026());
        let session = limits.session.expect("session");
        assert_eq!(session.percent_used, 41.0);
        assert_eq!(session.resets_at, 0, "unknown, not guessed");
        assert!(limits.week_all.is_some());
    }

    #[test]
    fn the_fixture_on_disk_still_parses() {
        let text = include_str!("../tests/fixtures/usage-output.txt");
        let limits = parse_limits(text, sep_10_2026());
        assert!(limits.session.is_some());
        assert!(limits.week_all.is_some());
        assert!(limits.week_model.is_some());
    }

    fn source() -> LimitsSource {
        LimitsSource::new(Duration::from_secs(180))
    }

    #[test]
    fn a_successful_read_is_stamped_with_its_time() {
        let mut source = source();
        source.apply(Ok(REPORT.to_string()), 1000);
        assert!(source.limits().is_some());
        assert_eq!(source.read_at(), Some(1000));
        assert_eq!(source.error(), None);
    }

    /// The widget must never blank out because one poll failed.
    #[test]
    fn a_failure_keeps_the_previous_reading() {
        let mut source = source();
        source.apply(Ok(REPORT.to_string()), 1000);
        source.apply(Err(UsageError::TimedOut), 2000);

        assert_eq!(
            source
                .limits()
                .expect("kept")
                .session
                .as_ref()
                .unwrap()
                .percent_used,
            41.0
        );
        assert_eq!(source.read_at(), Some(1000), "still the successful read");
        assert!(source.error().is_some());
    }

    #[test]
    fn a_later_success_clears_the_error() {
        let mut source = source();
        source.apply(Err(UsageError::NotFound), 1000);
        source.apply(Ok(REPORT.to_string()), 2000);
        assert_eq!(source.error(), None);
        assert_eq!(source.read_at(), Some(2000));
    }

    /// A report that no longer contains any limit line is a wording change,
    /// not a working reading of zero.
    #[test]
    fn an_unrecognized_report_counts_as_a_failure() {
        let mut source = source();
        source.apply(Ok("Everything is fine.".to_string()), 1000);
        assert!(source.limits().is_none());
        assert!(source.error().is_some());
    }

    /// What the command actually prints when nobody is signed in: no error,
    /// exit zero, and a cost summary where the limits should be.
    const SIGNED_OUT: &str = "Total cost:            $0.0000
Total duration (API):  0s
Total duration (wall): 0s
Total code changes:    0 lines added, 0 lines removed
Usage:                 0 input, 0 output, 0 cache read, 0 cache write";

    #[test]
    fn being_signed_out_is_named_rather_than_reported_as_a_parse_failure() {
        let mut source = source();
        source.apply(Ok(SIGNED_OUT.to_string()), 1000);

        assert_eq!(
            source.error().map(|e| e.code),
            Some(LimitsErrorCode::NotLoggedIn)
        );
        assert!(source.limits().is_none());
    }

    #[test]
    fn a_reworded_report_is_not_blamed_on_being_signed_out() {
        let mut source = source();
        source.apply(Ok("Your capacity looks great today!".to_string()), 1000);

        assert_eq!(
            source.error().map(|e| e.code),
            Some(LimitsErrorCode::ReportChanged)
        );
    }

    /// Signing out must not wipe a reading taken while signed in.
    #[test]
    fn signing_out_keeps_the_last_good_reading() {
        let mut source = source();
        source.apply(Ok(REPORT.to_string()), 1000);
        source.apply(Ok(SIGNED_OUT.to_string()), 2000);

        assert!(source.limits().is_some());
        assert_eq!(source.read_at(), Some(1000));
    }

    #[test]
    fn the_interval_widens_only_after_repeated_failures() {
        let mut source = source();
        let base = source.next_interval();

        source.apply(Err(UsageError::TimedOut), 1);
        source.apply(Err(UsageError::TimedOut), 2);
        assert_eq!(source.next_interval(), base, "two failures are noise");

        source.apply(Err(UsageError::TimedOut), 3);
        assert_eq!(source.next_interval(), base * 2);
        source.apply(Err(UsageError::TimedOut), 4);
        assert_eq!(source.next_interval(), base * 4);
    }

    #[test]
    fn the_backoff_stops_widening() {
        let mut source = source();
        for tick in 0..40 {
            source.apply(Err(UsageError::TimedOut), tick);
        }
        assert_eq!(source.next_interval(), MAX_INTERVAL);
    }

    #[test]
    fn a_success_resets_the_backoff() {
        let mut source = source();
        for tick in 0..5 {
            source.apply(Err(UsageError::TimedOut), tick);
        }
        source.apply(Ok(REPORT.to_string()), 10);
        assert_eq!(source.next_interval(), Duration::from_secs(180));
    }

    #[test]
    fn takes_the_report_out_of_the_envelope() {
        let stdout =
            r#"{"is_error":false,"result":"Current session: 41% used","total_cost_usd":0}"#;
        assert_eq!(
            extract_result(stdout).expect("result"),
            "Current session: 41% used"
        );
    }

    #[test]
    fn surrounding_whitespace_is_tolerated() {
        let stdout = "\n  {\"result\":\"ok\"}  \n";
        assert_eq!(extract_result(stdout).expect("result"), "ok");
    }

    #[test]
    fn an_error_envelope_is_an_error() {
        let stdout = r#"{"is_error":true,"result":"Not logged in"}"#;
        assert_eq!(
            extract_result(stdout),
            Err(UsageError::Unusable("Not logged in".into()))
        );
    }

    #[test]
    fn output_that_is_not_json_is_reported_rather_than_panicking() {
        assert!(matches!(
            extract_result("Not logged in · Please run /login"),
            Err(UsageError::Unusable(_))
        ));
    }

    #[test]
    fn a_missing_result_field_is_an_error() {
        assert!(matches!(
            extract_result(r#"{"is_error":false}"#),
            Err(UsageError::Unusable(_))
        ));
        assert!(matches!(
            extract_result(r#"{"result":"   "}"#),
            Err(UsageError::Unusable(_))
        ));
    }

    #[test]
    fn a_timeout_returns_rather_than_hanging() {
        let started = std::time::Instant::now();
        let outcome = run(Path::new(EXECUTABLE_NAME), Duration::from_millis(1));
        // Either the command was slower than a millisecond (the normal case) or
        // it is not installed; both must return promptly.
        assert!(matches!(
            outcome,
            Err(UsageError::TimedOut) | Err(UsageError::NotFound)
        ));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[cfg(unix)]
    fn fake_executable(dir: &Path, name: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        std::fs::create_dir_all(dir).expect("dir");
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\n").expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    #[test]
    #[cfg(unix)]
    fn the_native_installer_location_is_searched() {
        let home = tempfile::tempdir().expect("tempdir");
        let expected = fake_executable(&home.path().join(".local/bin"), EXECUTABLE_NAME);

        assert_eq!(
            first_executable(&known_locations(Some(home.path()))),
            Some(expected)
        );
    }

    #[test]
    #[cfg(unix)]
    fn a_home_without_claude_yields_nothing_from_the_home_locations() {
        let home = tempfile::tempdir().expect("tempdir");
        let found = known_locations(Some(home.path()))
            .into_iter()
            .filter(|path| path.starts_with(home.path()))
            .find(|path| is_executable(path));

        assert_eq!(found, None);
    }

    #[test]
    #[cfg(unix)]
    fn the_native_installer_wins_over_an_npm_install() {
        let home = tempfile::tempdir().expect("tempdir");
        let native = fake_executable(&home.path().join(".local/bin"), EXECUTABLE_NAME);
        fake_executable(
            &home.path().join(".nvm/versions/node/v24.14.1/bin"),
            EXECUTABLE_NAME,
        );

        assert_eq!(
            first_executable(&known_locations(Some(home.path()))),
            Some(native)
        );
    }

    #[test]
    #[cfg(unix)]
    fn an_npm_install_under_nvm_is_found() {
        let home = tempfile::tempdir().expect("tempdir");
        let expected = fake_executable(
            &home.path().join(".nvm/versions/node/v24.14.1/bin"),
            EXECUTABLE_NAME,
        );

        assert_eq!(
            first_executable(&known_locations(Some(home.path()))),
            Some(expected)
        );
    }

    #[test]
    #[cfg(unix)]
    fn several_node_versions_are_tried_in_a_fixed_order() {
        let home = tempfile::tempdir().expect("tempdir");
        for version in ["v20.0.0", "v22.1.0", "v24.14.1"] {
            fake_executable(
                &home
                    .path()
                    .join(".nvm/versions/node")
                    .join(version)
                    .join("bin"),
                EXECUTABLE_NAME,
            );
        }

        let twice = (
            first_executable(&known_locations(Some(home.path()))),
            first_executable(&known_locations(Some(home.path()))),
        );

        assert_eq!(twice.0, twice.1);
        assert!(twice.0.is_some());
    }

    #[test]
    #[cfg(unix)]
    fn a_directory_named_claude_is_not_taken_for_the_executable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let trap = dir.path().join(".local/bin").join(EXECUTABLE_NAME);
        std::fs::create_dir_all(&trap).expect("dir");

        assert!(!is_executable(&trap));
        assert_eq!(first_executable(&[trap]), None);
    }

    #[test]
    #[cfg(unix)]
    fn a_file_without_an_execute_bit_is_skipped() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(EXECUTABLE_NAME);
        std::fs::write(&path, "#!/bin/sh\n").expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");

        assert!(!is_executable(&path));
    }

    #[test]
    #[cfg(unix)]
    fn an_rc_file_banner_is_not_mistaken_for_a_path() {
        let stdout = "Welcome back!\n  nvm: using v24\n/Users/x/.local/bin/claude\n";

        assert_eq!(
            shell_output_paths(stdout),
            vec![PathBuf::from("/Users/x/.local/bin/claude")]
        );
    }

    #[test]
    #[cfg(unix)]
    fn a_shell_that_found_nothing_yields_no_path() {
        assert_eq!(
            shell_output_paths("claude not found\n"),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn the_command_runs_beside_the_settings_file_rather_than_wherever_it_started() {
        let settings = Path::new("/Users/x/Library/Application Support/cuw/settings.json");

        assert_eq!(
            working_dir_from(settings),
            Some(PathBuf::from(
                "/Users/x/Library/Application Support/cuw/usage-cwd"
            ))
        );
    }

    #[test]
    fn a_configured_path_is_used_without_searching() {
        let mut source = source();
        source.set_claude_path(Some(PathBuf::from("/opt/claude/claude")));

        assert_eq!(source.program(), Ok(PathBuf::from("/opt/claude/claude")));
    }

    #[test]
    fn changing_the_configured_path_discards_what_the_search_found() {
        let mut source = source();
        source.resolved = Some(PathBuf::from("/usr/local/bin/claude"));

        source.set_claude_path(Some(PathBuf::from("/opt/claude/claude")));

        assert_eq!(source.resolved, None);
    }

    #[test]
    fn setting_the_same_path_again_keeps_the_cache() {
        let mut source = source();
        source.set_claude_path(Some(PathBuf::from("/opt/claude/claude")));
        source.resolved = Some(PathBuf::from("/usr/local/bin/claude"));

        source.set_claude_path(Some(PathBuf::from("/opt/claude/claude")));

        assert_eq!(
            source.resolved,
            Some(PathBuf::from("/usr/local/bin/claude"))
        );
    }

    #[test]
    fn a_cached_path_is_returned_rather_than_searched_for_again() {
        let mut source = source();
        source.resolved = Some(PathBuf::from("/usr/local/bin/claude"));

        assert_eq!(source.program(), Ok(PathBuf::from("/usr/local/bin/claude")));
    }

    /// Reports where the search lands on the machine it runs on, so a failure
    /// to find the command can be told apart from a failure to run it. Run the
    /// test binary under the PATH a bundled app is given to check the layers
    /// that do not depend on the shell.
    #[test]
    #[ignore = "reports what is installed on this machine"]
    #[cfg(unix)]
    fn where_the_search_lands() {
        println!("PATH={:?}", std::env::var_os("PATH"));
        println!("inherited PATH: {:?}", search_inherited_path());
        println!(
            "known locations: {:?}",
            first_executable(&known_locations(home().as_deref()))
        );
        println!("login shell: {:?}", ask_login_shell());
        println!("resolved: {:?}", resolve());
    }

    /// Spawns the real CLI, so it is excluded from the default run.
    #[test]
    #[ignore = "runs the installed claude CLI"]
    fn the_real_command_reports_limits() {
        let program = resolve().expect("claude on this machine");
        let text = read_usage(&program, DEFAULT_TIMEOUT).expect("usage");
        println!("{text}");
        assert!(
            text.contains("Current session:"),
            "unexpected report: {text}"
        );
    }
}
