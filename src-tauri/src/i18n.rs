//! Wording for the parts the operating system draws.
//!
//! The widget translates itself in the frontend; this covers only what Rust
//! hands to the shell — tray menu, tooltip, notifications — where there is no
//! DOM to re-render.

use crate::settings::Language;

/// Tray menu and notification wording.
pub struct Strings {
    pub open_settings: &'static str,
    pub refresh_now: &'static str,
    pub quit: &'static str,
    /// Title bar of the settings window, which the shell draws.
    pub settings_title: &'static str,
    /// Prefix for today's cost in the tooltip, e.g. "Hôm nay".
    pub today: &'static str,
    /// Suffix marking the figure as a conversion, not a charge.
    pub at_api_rates: &'static str,
    pub session: &'static str,
    pub week: &'static str,
    /// Notification body when the current rate runs out before the reset.
    pub will_run_out: &'static str,
    /// Notification body when the threshold is passed but the pace is fine.
    pub past_threshold: &'static str,
}

const VI: Strings = Strings {
    open_settings: "Mở cấu hình",
    refresh_now: "Đọc lại ngay",
    quit: "Thoát",
    settings_title: "Cấu hình",
    today: "Hôm nay",
    at_api_rates: "quy đổi theo giá API",
    session: "Phiên",
    week: "Tuần",
    will_run_out: "Theo nhịp hiện tại sẽ hết hạn mức trước khi reset",
    past_threshold: "Đã vượt mức cảnh báo bạn đặt",
};

const EN: Strings = Strings {
    open_settings: "Open settings",
    refresh_now: "Refresh now",
    quit: "Quit",
    settings_title: "Settings",
    today: "Today",
    at_api_rates: "at API rates",
    session: "Session",
    week: "Week",
    will_run_out: "At this rate it runs out before the reset",
    past_threshold: "Past the threshold you set",
};

pub fn strings(language: Language) -> &'static Strings {
    match language {
        Language::Vi => &VI,
        Language::En => &EN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every field must differ between the two, or one was left untranslated.
    #[test]
    fn both_languages_are_filled_in() {
        let vi = strings(Language::Vi);
        let en = strings(Language::En);
        let pairs = [
            (vi.open_settings, en.open_settings),
            (vi.refresh_now, en.refresh_now),
            (vi.quit, en.quit),
            (vi.settings_title, en.settings_title),
            (vi.today, en.today),
            (vi.at_api_rates, en.at_api_rates),
            (vi.session, en.session),
            (vi.week, en.week),
            (vi.will_run_out, en.will_run_out),
            (vi.past_threshold, en.past_threshold),
        ];
        for (a, b) in pairs {
            assert!(!a.is_empty() && !b.is_empty());
            assert_ne!(a, b, "left untranslated: {a}");
        }
    }
}
