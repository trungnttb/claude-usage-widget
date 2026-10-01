//! Model prices and cost math.
//!
//! Prices are USD per 1M tokens from the published Anthropic table (2026-06-24).
//! Only base input and output rates are published per model; the cache tiers are
//! derived from the documented multipliers below.

/// Cache reads bill at a fraction of the model's own input rate.
const CACHE_READ_MULTIPLIER: f64 = 0.1;
const CACHE_WRITE_5M_MULTIPLIER: f64 = 1.25;
const CACHE_WRITE_1H_MULTIPLIER: f64 = 2.0;

/// Applied to any model absent from the table, so a newly released model is
/// priced approximately rather than silently counted as free.
const FALLBACK: Price = Price {
    input: 5.0,
    output: 25.0,
    cache_read_multiplier: None,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    /// Set only where the model prices cache reads off the shared multiplier.
    pub cache_read_multiplier: Option<f64>,
}

impl Price {
    const fn new(input: f64, output: f64) -> Self {
        Self {
            input,
            output,
            cache_read_multiplier: None,
        }
    }

    fn cache_read_rate(&self) -> f64 {
        self.input * self.cache_read_multiplier.unwrap_or(CACHE_READ_MULTIPLIER)
    }
}

const PRICES: &[(&str, Price)] = &[
    (
        "claude-fable-5-1",
        Price {
            input: 10.0,
            output: 50.0,
            cache_read_multiplier: Some(0.025),
        },
    ),
    ("claude-mythos-5-1", Price::new(10.0, 50.0)),
    ("claude-fable-5", Price::new(10.0, 50.0)),
    ("claude-mythos-5", Price::new(10.0, 50.0)),
    ("claude-opus-5", Price::new(5.0, 25.0)),
    ("claude-opus-4-8", Price::new(5.0, 25.0)),
    ("claude-opus-4-7", Price::new(5.0, 25.0)),
    ("claude-opus-4-6", Price::new(5.0, 25.0)),
    ("claude-opus-4-5", Price::new(5.0, 25.0)),
    ("claude-sonnet-5", Price::new(2.0, 10.0)),
    ("claude-sonnet-4-6", Price::new(3.0, 15.0)),
    ("claude-sonnet-4-5", Price::new(3.0, 15.0)),
    ("claude-haiku-4-5", Price::new(1.0, 5.0)),
];

/// Fast mode runs the same model at premium rates. Opus 5 and Opus 4.8 only.
const FAST_MODE_PRICES: &[(&str, Price)] = &[
    ("claude-opus-5", Price::new(10.0, 50.0)),
    ("claude-opus-4-8", Price::new(10.0, 50.0)),
];

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TokenCounts {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cost {
    pub usd: f64,
    /// False when the model was absent from the table and priced by fallback.
    pub exact: bool,
}

/// Transcript model ids carry decorations the price table does not use: a
/// context marker (`claude-opus-5[1m]`) or a dated snapshot
/// (`claude-haiku-4-5-20251001`, `claude-opus-4-5@20251101`).
///
/// Returns `None` for synthetic entries such as `<synthetic>`, which carry no
/// billable usage.
pub fn normalize_model_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with('<') {
        return None;
    }

    let mut id = trimmed.to_ascii_lowercase();

    if id.ends_with(']') {
        if let Some(open) = id.rfind('[') {
            id.truncate(open);
        }
    }

    id = strip_date_suffix(&id);

    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

/// Removes a trailing `-YYYYMMDD` or `@YYYYMMDD` snapshot marker.
fn strip_date_suffix(id: &str) -> String {
    let bytes = id.as_bytes();
    if bytes.len() < 9 {
        return id.to_string();
    }
    let split = bytes.len() - 9;
    let separator = bytes[split];
    if separator != b'-' && separator != b'@' {
        return id.to_string();
    }
    if bytes[split + 1..].iter().all(u8::is_ascii_digit) {
        id[..split].to_string()
    } else {
        id.to_string()
    }
}

fn lookup(table: &[(&str, Price)], id: &str) -> Option<Price> {
    table
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, price)| *price)
}

/// `speed` is the `usage.speed` field from the transcript: `standard` or `fast`.
pub fn price_for(model: &str, speed: Option<&str>) -> Option<(Price, bool)> {
    let id = normalize_model_id(model)?;

    if speed == Some("fast") {
        if let Some(price) = lookup(FAST_MODE_PRICES, &id) {
            return Some((price, true));
        }
    }

    match lookup(PRICES, &id) {
        Some(price) => Some((price, true)),
        None => Some((FALLBACK, false)),
    }
}

pub fn cost_of(model: &str, tokens: &TokenCounts, speed: Option<&str>) -> Cost {
    let Some((price, exact)) = price_for(model, speed) else {
        return Cost {
            usd: 0.0,
            exact: true,
        };
    };

    let per_million = tokens.input as f64 * price.input
        + tokens.output as f64 * price.output
        + tokens.cache_read as f64 * price.cache_read_rate()
        + tokens.cache_write_5m as f64 * price.input * CACHE_WRITE_5M_MULTIPLIER
        + tokens.cache_write_1h as f64 * price.input * CACHE_WRITE_1H_MULTIPLIER;

    Cost {
        usd: per_million / 1_000_000.0,
        exact,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_context_marker() {
        assert_eq!(
            normalize_model_id("claude-opus-5[1m]").as_deref(),
            Some("claude-opus-5")
        );
    }

    #[test]
    fn strips_dated_snapshot() {
        assert_eq!(
            normalize_model_id("claude-haiku-4-5-20251001").as_deref(),
            Some("claude-haiku-4-5")
        );
        assert_eq!(
            normalize_model_id("claude-opus-4-5@20251101").as_deref(),
            Some("claude-opus-4-5")
        );
    }

    #[test]
    fn keeps_version_digits_that_are_not_a_date() {
        assert_eq!(
            normalize_model_id("claude-opus-4-8").as_deref(),
            Some("claude-opus-4-8")
        );
    }

    #[test]
    fn rejects_synthetic_entries() {
        assert_eq!(normalize_model_id("<synthetic>"), None);
        assert_eq!(normalize_model_id("  "), None);
    }

    /// Values taken from a real transcript record on claude-opus-5.
    #[test]
    fn prices_a_real_record() {
        let tokens = TokenCounts {
            input: 2,
            output: 268,
            cache_read: 33_316,
            cache_write_1h: 25_263,
            ..Default::default()
        };
        let cost = cost_of("claude-opus-5", &tokens, Some("standard"));
        assert!(cost.exact);
        assert!(
            (cost.usd - 0.275_998).abs() < 1e-9,
            "got {} want 0.275998",
            cost.usd
        );
    }

    #[test]
    fn fast_mode_doubles_opus_rates() {
        let tokens = TokenCounts {
            output: 1_000_000,
            ..Default::default()
        };
        assert_eq!(
            cost_of("claude-opus-5", &tokens, Some("standard")).usd,
            25.0
        );
        assert_eq!(cost_of("claude-opus-5", &tokens, Some("fast")).usd, 50.0);
    }

    #[test]
    fn fast_mode_falls_back_to_standard_rates_for_other_models() {
        let tokens = TokenCounts {
            output: 1_000_000,
            ..Default::default()
        };
        assert_eq!(cost_of("claude-sonnet-5", &tokens, Some("fast")).usd, 10.0);
    }

    #[test]
    fn fable_prices_cache_reads_lower_than_the_shared_multiplier() {
        let tokens = TokenCounts {
            cache_read: 1_000_000,
            ..Default::default()
        };
        assert_eq!(cost_of("claude-fable-5-1", &tokens, None).usd, 0.25);
        assert_eq!(cost_of("claude-fable-5", &tokens, None).usd, 1.0);
    }

    #[test]
    fn cache_writes_price_by_ttl() {
        let tokens = TokenCounts {
            cache_write_5m: 1_000_000,
            ..Default::default()
        };
        assert_eq!(cost_of("claude-opus-5", &tokens, None).usd, 6.25);

        let tokens = TokenCounts {
            cache_write_1h: 1_000_000,
            ..Default::default()
        };
        assert_eq!(cost_of("claude-opus-5", &tokens, None).usd, 10.0);
    }

    #[test]
    fn unknown_model_is_priced_but_flagged() {
        let tokens = TokenCounts {
            output: 1_000_000,
            ..Default::default()
        };
        let cost = cost_of("claude-something-9", &tokens, None);
        assert!(!cost.exact);
        assert_eq!(cost.usd, 25.0);
    }

    #[test]
    fn synthetic_records_cost_nothing() {
        let tokens = TokenCounts {
            output: 1_000_000,
            ..Default::default()
        };
        let cost = cost_of("<synthetic>", &tokens, None);
        assert_eq!(cost.usd, 0.0);
        assert!(cost.exact);
    }
}
