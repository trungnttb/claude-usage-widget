//! Reads Claude Code transcripts and turns them into billable usage records.
//!
//! One API response is written to the transcript as several lines — one per
//! content block — and every one of them repeats the same `usage` object.
//! Measured on a real directory, 48.9% of the usage-bearing lines are such
//! repeats, so deduplication is not an optimization: without it every reported
//! figure is roughly doubled.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::DateTime;
use serde::Deserialize;

use crate::pricing::{self, TokenCounts};

#[derive(Debug, Clone, PartialEq)]
pub struct UsageRecord {
    /// Unix seconds.
    pub timestamp: i64,
    pub model: String,
    pub tokens: TokenCounts,
    pub cost_usd: f64,
    /// False when the model was absent from the price table.
    pub exact_pricing: bool,
    /// Last path component of the record's `cwd`.
    pub project: String,
    pub session_id: String,
}

#[derive(Debug, Default)]
pub struct ScanResult {
    pub records: Vec<UsageRecord>,
    pub duplicates_skipped: usize,
    pub malformed_lines: usize,
    pub files_read: usize,
}

/// `~/.claude/projects`, or None when the home directory cannot be determined.
pub fn default_projects_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    Some(home.join(".claude").join("projects"))
}

/// Walks `root` once and collects every deduplicated usage record.
///
/// For repeated reads use [`Scanner`], which only reads what was appended since
/// the previous pass.
pub fn scan_dir(root: &Path) -> ScanResult {
    let mut scanner = Scanner::new(root.to_path_buf());
    scanner.scan();
    scanner.into_result()
}

/// Where reading of one transcript file stopped.
#[derive(Debug, Default)]
struct FileCursor {
    /// Bytes already consumed, so the next pass seeks straight past them.
    offset: u64,
    /// Trailing bytes after the last newline. Claude Code appends to these
    /// files while we read, so the final line is regularly incomplete; holding
    /// it back and prepending it next pass is what keeps that record intact.
    partial: Vec<u8>,
}

/// Reads a transcript directory repeatedly, parsing only appended bytes.
///
/// A full pass over ~100 MB costs about 200 ms, which is too much to repeat on
/// a few-second refresh; after the first pass this reads only what is new.
pub struct Scanner {
    root: PathBuf,
    cursors: HashMap<PathBuf, FileCursor>,
    seen: HashSet<String>,
    records: Vec<UsageRecord>,
    duplicates_skipped: usize,
    malformed_lines: usize,
}

/// What one [`Scanner::scan`] pass added.
#[derive(Debug, Default, PartialEq)]
pub struct ScanDelta {
    pub new_records: usize,
    pub files_touched: usize,
    /// Files that shrank or were replaced and had to be re-read from the start.
    pub files_restarted: usize,
}

impl Scanner {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            cursors: HashMap::new(),
            seen: HashSet::new(),
            records: Vec::new(),
            duplicates_skipped: 0,
            malformed_lines: 0,
        }
    }

    pub fn records(&self) -> &[UsageRecord] {
        &self.records
    }

    pub fn into_result(self) -> ScanResult {
        ScanResult {
            files_read: self.cursors.len(),
            records: self.records,
            duplicates_skipped: self.duplicates_skipped,
            malformed_lines: self.malformed_lines,
        }
    }

    /// Reads whatever has been appended since the previous call.
    pub fn scan(&mut self) -> ScanDelta {
        let mut delta = ScanDelta::default();
        let before = self.records.len();

        let mut files = Vec::new();
        collect_jsonl(&self.root, &mut files);
        let present: HashSet<PathBuf> = files.iter().cloned().collect();
        self.cursors.retain(|path, _| present.contains(path));

        for path in files {
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            let size = metadata.len();
            let cursor = self.cursors.entry(path.clone()).or_default();

            // A file smaller than where we stopped was truncated or replaced,
            // so anything remembered about it no longer describes its content.
            if size < cursor.offset {
                *cursor = FileCursor::default();
                delta.files_restarted += 1;
            }
            if size == cursor.offset {
                continue;
            }

            let Ok(appended) = read_from(&path, cursor.offset) else {
                continue;
            };
            cursor.offset += appended.len() as u64;
            delta.files_touched += 1;

            let mut buffer = std::mem::take(&mut cursor.partial);
            buffer.extend_from_slice(&appended);
            let complete_upto = terminated_prefix(&buffer);
            if complete_upto == 0 {
                // Nothing usable yet; hold everything for the next pass.
                cursor.partial = buffer;
                continue;
            }
            cursor.partial = buffer[complete_upto..].to_vec();

            let text = String::from_utf8_lossy(&buffer[..complete_upto]);
            let mut result = ScanResult::default();
            read_lines(&text, &mut self.seen, &mut result);
            self.duplicates_skipped += result.duplicates_skipped;
            self.malformed_lines += result.malformed_lines;
            self.records.extend(result.records);
        }

        delta.new_records = self.records.len() - before;
        delta
    }

    /// Drops records older than `cutoff` (unix seconds) so a long-running
    /// process does not accumulate the entire history in memory. Deduplication
    /// keys are kept, since re-admitting an old record would corrupt the totals.
    pub fn prune_before(&mut self, cutoff: i64) {
        self.records
            .retain(|record| record.timestamp == 0 || record.timestamp >= cutoff);
    }
}

/// How many bytes of `buffer` are safe to parse now.
///
/// Everything up to the last newline always is. The bytes after it are the
/// awkward case: a transcript file whose writer has finished often has no
/// trailing newline, and holding that tail back would lose one record per
/// session permanently — but a file still being written ends mid-record, and
/// parsing that would produce a bogus one. Valid JSON tells the two apart,
/// because a half-written object does not parse.
fn terminated_prefix(buffer: &[u8]) -> usize {
    let after_newline = buffer
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);

    let tail = &buffer[after_newline..];
    if !tail.is_empty() && serde_json::from_slice::<serde::de::IgnoredAny>(tail).is_ok() {
        return buffer.len();
    }
    after_newline
}

fn read_from(path: &Path, offset: u64) -> std::io::Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn collect_jsonl(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "jsonl") {
            out.push(path);
        }
    }
}

/// Parses the usage-bearing lines of one transcript chunk into `result`.
///
/// `seen` is threaded across files because the caller needs one deduplication
/// domain for the whole scan.
pub fn read_lines(text: &str, seen: &mut HashSet<String>, result: &mut ScanResult) {
    for line in text.lines() {
        // Most lines carry no usage at all; skipping them before serde runs is
        // what keeps a full scan of ~100 MB inside a few hundred milliseconds.
        if !line.contains("\"usage\"") {
            continue;
        }

        let parsed: TranscriptLine = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => {
                result.malformed_lines += 1;
                continue;
            }
        };

        let Some(message) = parsed.message else {
            continue;
        };
        let Some(usage) = message.usage else { continue };

        let key = format!(
            "{}:{}",
            message.id.as_deref().unwrap_or(""),
            parsed.request_id.as_deref().unwrap_or("")
        );
        if !seen.insert(key) {
            result.duplicates_skipped += 1;
            continue;
        }

        let model = message.model.unwrap_or_default();
        // Synthetic entries carry no billable usage and would pollute the
        // per-model breakdown.
        if pricing::normalize_model_id(&model).is_none() {
            continue;
        }

        let tokens = usage.token_counts();
        let cost = pricing::cost_of(&model, &tokens, usage.speed.as_deref());

        result.records.push(UsageRecord {
            timestamp: parsed
                .timestamp
                .as_deref()
                .and_then(parse_rfc3339)
                .unwrap_or(0),
            model,
            tokens,
            cost_usd: cost.usd,
            exact_pricing: cost.exact,
            project: parsed.cwd.as_deref().map(project_name).unwrap_or_default(),
            session_id: parsed.session_id.unwrap_or_default(),
        });
    }
}

fn parse_rfc3339(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.timestamp())
}

/// Transcript `cwd` values use the host separator, so both are accepted.
fn project_name(cwd: &str) -> String {
    cwd.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_string()
}

#[derive(Deserialize)]
struct TranscriptLine {
    message: Option<Message>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    timestamp: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}

#[derive(Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Usage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    cache_creation: Option<CacheCreation>,
    speed: Option<String>,
}

#[derive(Deserialize)]
struct CacheCreation {
    #[serde(default)]
    ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    ephemeral_1h_input_tokens: u64,
}

impl Usage {
    fn token_counts(&self) -> TokenCounts {
        // Records predating the per-TTL breakdown report only a total. The
        // documented default TTL is 5 minutes, so that is where the total goes.
        let (write_5m, write_1h) = match &self.cache_creation {
            Some(split) => (
                split.ephemeral_5m_input_tokens,
                split.ephemeral_1h_input_tokens,
            ),
            None => (self.cache_creation_input_tokens, 0),
        };

        TokenCounts {
            input: self.input_tokens,
            output: self.output_tokens,
            cache_read: self.cache_read_input_tokens,
            cache_write_5m: write_5m,
            cache_write_1h: write_1h,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(text: &str) -> ScanResult {
        let mut result = ScanResult::default();
        let mut seen = HashSet::new();
        read_lines(text, &mut seen, &mut result);
        result
    }

    const ONE: &str = r#"{"message":{"id":"msg_1","model":"claude-opus-5","usage":{"input_tokens":2,"output_tokens":268,"cache_read_input_tokens":33316,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":25263},"speed":"standard"}},"requestId":"req_1","timestamp":"2026-09-09T04:06:01.398Z","cwd":"C:\\Users\\me\\workspace\\jb-e2e","sessionId":"sess_1"}"#;

    #[test]
    fn reads_one_record() {
        let result = scan(ONE);
        assert_eq!(result.records.len(), 1);
        let record = &result.records[0];
        assert_eq!(record.model, "claude-opus-5");
        assert_eq!(record.project, "jb-e2e");
        assert_eq!(record.session_id, "sess_1");
        assert_eq!(record.tokens.cache_read, 33_316);
        assert_eq!(record.tokens.cache_write_1h, 25_263);
        assert!((record.cost_usd - 0.275_998).abs() < 1e-9);
        assert!(record.exact_pricing);
    }

    /// The defining behaviour of this module: one API response repeated across
    /// content-block lines must be counted once.
    #[test]
    fn counts_a_repeated_response_once() {
        let text = format!("{ONE}\n{ONE}\n{ONE}");
        let result = scan(&text);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.duplicates_skipped, 2);
    }

    #[test]
    fn separate_responses_are_kept_apart() {
        let other = ONE.replace("msg_1", "msg_2").replace("req_1", "req_2");
        let result = scan(&format!("{ONE}\n{other}"));
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.duplicates_skipped, 0);
    }

    #[test]
    fn a_broken_line_does_not_stop_the_scan() {
        let text = format!("{{\"usage\": broken\n{ONE}");
        let result = scan(&text);
        assert_eq!(result.malformed_lines, 1);
        assert_eq!(result.records.len(), 1);
    }

    #[test]
    fn lines_without_usage_are_ignored() {
        let text = format!("{{\"type\":\"user\",\"message\":{{\"role\":\"user\"}}}}\n{ONE}");
        let result = scan(&text);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.malformed_lines, 0);
    }

    #[test]
    fn synthetic_records_are_dropped() {
        let text = ONE
            .replace("claude-opus-5", "<synthetic>")
            .replace("msg_1", "msg_s");
        let result = scan(&text);
        assert!(result.records.is_empty());
    }

    #[test]
    fn a_record_without_the_ttl_split_bills_the_total_at_the_default_ttl() {
        let text = r#"{"message":{"id":"m","model":"claude-opus-5","usage":{"cache_creation_input_tokens":1000000}},"requestId":"r"}"#;
        let result = scan(text);
        assert_eq!(result.records[0].tokens.cache_write_5m, 1_000_000);
        assert_eq!(result.records[0].tokens.cache_write_1h, 0);
        assert_eq!(result.records[0].cost_usd, 6.25);
    }

    #[test]
    fn an_unparsable_timestamp_does_not_drop_the_record() {
        let text = ONE.replace("2026-09-09T04:06:01.398Z", "not a date");
        let result = scan(&text);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].timestamp, 0);
    }

    #[test]
    fn timestamps_become_unix_seconds() {
        let result = scan(ONE);
        assert_eq!(result.records[0].timestamp, 1_788_926_761);
    }

    #[test]
    fn project_name_handles_both_separators() {
        assert_eq!(project_name("C:\\Users\\me\\api"), "api");
        assert_eq!(project_name("/home/me/api"), "api");
        assert_eq!(project_name("/home/me/api/"), "api");
    }

    fn other(tag: &str) -> String {
        ONE.replace("msg_1", &format!("msg_{tag}"))
            .replace("req_1", &format!("req_{tag}"))
    }

    #[test]
    fn a_second_pass_reads_only_what_was_appended() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("s.jsonl");
        fs::write(&file, format!("{ONE}\n")).expect("write");

        let mut scanner = Scanner::new(dir.path().to_path_buf());
        assert_eq!(scanner.scan().new_records, 1);

        // Nothing changed, so nothing is re-read.
        let delta = scanner.scan();
        assert_eq!(delta.new_records, 0);
        assert_eq!(delta.files_touched, 0);

        let mut appended = fs::read_to_string(&file).expect("read");
        appended.push_str(&format!("{}\n", other("b")));
        fs::write(&file, appended).expect("append");

        let delta = scanner.scan();
        assert_eq!(delta.new_records, 1);
        assert_eq!(delta.files_touched, 1);
        assert_eq!(scanner.records().len(), 2);
    }

    /// Claude Code appends while we read, so a pass regularly ends mid-line.
    #[test]
    fn a_line_split_across_two_passes_is_still_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("s.jsonl");
        let record = other("split");
        let (head, tail) = record.split_at(record.len() / 2);
        fs::write(&file, head).expect("write head");

        let mut scanner = Scanner::new(dir.path().to_path_buf());
        assert_eq!(scanner.scan().new_records, 0, "half a line is not a record");
        assert_eq!(
            scanner.malformed_lines, 0,
            "an unterminated line is not an error"
        );

        fs::write(&file, format!("{head}{tail}\n")).expect("write whole");
        assert_eq!(scanner.scan().new_records, 1);
    }

    #[test]
    fn a_truncated_file_is_read_again_from_the_start() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("s.jsonl");
        fs::write(&file, format!("{ONE}\n{}\n", other("b"))).expect("write");

        let mut scanner = Scanner::new(dir.path().to_path_buf());
        assert_eq!(scanner.scan().new_records, 2);

        // Replaced by a shorter file holding a different record.
        fs::write(&file, format!("{}\n", other("c"))).expect("truncate");
        let delta = scanner.scan();
        assert_eq!(delta.files_restarted, 1);
        assert_eq!(delta.new_records, 1);
    }

    #[test]
    fn a_deleted_file_is_forgotten() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("s.jsonl");
        fs::write(&file, format!("{ONE}\n")).expect("write");

        let mut scanner = Scanner::new(dir.path().to_path_buf());
        scanner.scan();
        fs::remove_file(&file).expect("remove");

        assert_eq!(scanner.scan(), ScanDelta::default());
        assert!(scanner.cursors.is_empty());
    }

    #[test]
    fn a_file_appearing_later_is_picked_up() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut scanner = Scanner::new(dir.path().to_path_buf());
        assert_eq!(scanner.scan().new_records, 0);

        fs::write(dir.path().join("new.jsonl"), format!("{ONE}\n")).expect("write");
        assert_eq!(scanner.scan().new_records, 1);
    }

    #[test]
    fn pruning_drops_old_records_but_still_rejects_them_later() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("s.jsonl");
        fs::write(&file, format!("{ONE}\n")).expect("write");

        let mut scanner = Scanner::new(dir.path().to_path_buf());
        scanner.scan();
        assert_eq!(scanner.records().len(), 1);

        scanner.prune_before(i64::MAX);
        assert!(scanner.records().is_empty());

        // Re-reading the same file must not resurrect the pruned record.
        fs::write(&file, format!("{ONE}\n{ONE}\n")).expect("rewrite");
        scanner.scan();
        assert!(scanner.records().is_empty());
    }

    /// Runs against the developer's own transcripts, so it is excluded from the
    /// default run. Invoke with `cargo test -- --ignored real_directory`.
    #[test]
    #[ignore = "reads the live ~/.claude/projects directory"]
    fn real_directory_scans_within_the_refresh_interval() {
        let Some(root) = default_projects_dir() else {
            panic!("no home directory");
        };
        if !root.exists() {
            panic!("{} does not exist", root.display());
        }

        let started = std::time::Instant::now();
        let result = scan_dir(&root);
        let elapsed = started.elapsed();

        let total: f64 = result.records.iter().map(|r| r.cost_usd).sum();
        let estimated = result.records.iter().filter(|r| !r.exact_pricing).count();
        println!(
            "files={} records={} duplicates={} malformed={} estimated_pricing={} total=${:.2} elapsed={:?}",
            result.files_read,
            result.records.len(),
            result.duplicates_skipped,
            result.malformed_lines,
            estimated,
            total,
            elapsed
        );

        assert!(!result.records.is_empty(), "no records found");
        assert!(
            elapsed < std::time::Duration::from_secs(1),
            "scan took {elapsed:?}, over the 1s budget"
        );
    }

    #[test]
    fn scans_a_directory_tree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let nested = dir.path().join("project-a");
        fs::create_dir_all(&nested).expect("create nested dir");
        fs::write(nested.join("a.jsonl"), format!("{ONE}\n{ONE}")).expect("write a");
        fs::write(
            dir.path().join("b.jsonl"),
            ONE.replace("msg_1", "msg_b").replace("req_1", "req_b"),
        )
        .expect("write b");
        fs::write(dir.path().join("ignored.txt"), ONE).expect("write txt");

        let result = scan_dir(dir.path());
        assert_eq!(result.files_read, 2);
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.duplicates_skipped, 1);
    }
}
