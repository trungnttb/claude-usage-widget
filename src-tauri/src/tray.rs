//! The tray icon, its menu, and the number it shows.

use std::sync::Mutex;

#[cfg(not(target_os = "macos"))]
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

use crate::i18n;
use crate::settings::{Language, Settings, TrayMetric};
use crate::snapshot::Snapshot;
#[cfg(not(target_os = "macos"))]
use crate::tray_icon;
use crate::tray_icon::Level;
use crate::window::WIDGET_LABEL;

/// The text currently drawn, so an unchanged number costs no work.
static LAST_LABEL: Mutex<Option<String>> = Mutex::new(None);

pub const OPEN_SETTINGS: &str = "open-settings";
pub const REFRESH_NOW: &str = "refresh-now";
pub const QUIT: &str = "quit";

fn menu_for(app: &tauri::AppHandle, language: Language) -> tauri::Result<Menu<tauri::Wry>> {
    let words = i18n::strings(language);
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, OPEN_SETTINGS, words.open_settings, true, None::<&str>)?,
            &MenuItem::with_id(app, REFRESH_NOW, words.refresh_now, true, None::<&str>)?,
            &MenuItem::with_id(app, QUIT, words.quit, true, None::<&str>)?,
        ],
    )
}

/// Rebuilds the menu in the chosen language.
///
/// Menu labels live in the shell, not in a document, so changing language has
/// to hand it a new menu rather than re-render one.
pub fn set_language(app: &tauri::AppHandle, language: Language) {
    let (Some(tray), Ok(menu)) = (app.tray_by_id("main"), menu_for(app, language)) else {
        return;
    };
    let _ = tray.set_menu(Some(menu));
}

pub fn build(app: &tauri::AppHandle, language: Language) -> tauri::Result<()> {
    let menu = menu_for(app, language)?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundled icon"))
        .tooltip("Claude Usage")
        .menu(&menu)
        // Left click belongs to showing the widget, so the menu is right-click
        // only. The default on Windows would open the menu on both.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN_SETTINGS => crate::open_settings_from_tray(app),
            REFRESH_NOW => crate::refresh_from_tray(app),
            QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_widget(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Session above, week below, when that is what is configured.
///
/// Both are needed: showing one of a pair would read as the wrong figure.
pub fn stacked(snapshot: &Snapshot, settings: &Settings) -> Option<[(String, Level); 2]> {
    if settings.tray_metric != TrayMetric::SessionAndWeek {
        return None;
    }
    let limits = snapshot.limits.as_ref()?;
    let session = limits.session.as_ref()?;
    let week = limits.week_all.as_ref()?;
    Some([
        percent_label(session.percent_used),
        percent_label(week.percent_used),
    ])
}

/// What the tray shows, and the level that colours it.
///
/// Returns None when the chosen metric has no value yet, so the icon is left
/// as it was rather than flashing a placeholder.
pub fn label(snapshot: &Snapshot, settings: &Settings) -> Option<(String, Level)> {
    metric(snapshot, settings)
}

/// Characters that still render legibly in a 32 pixel icon. The limit belongs
/// to the bitmap alone: as menu bar text there is room for the whole figure,
/// and cutting it there would turn `12.3M` into `12.`.
#[cfg(not(target_os = "macos"))]
const MAX_GLYPHS: usize = 3;

#[cfg(not(target_os = "macos"))]
fn within_icon(text: &str) -> &str {
    match text.char_indices().nth(MAX_GLYPHS) {
        Some((end, _)) => &text[..end],
        None => text,
    }
}

fn metric(snapshot: &Snapshot, settings: &Settings) -> Option<(String, Level)> {
    match settings.tray_metric {
        TrayMetric::SessionPercent => {
            let window = snapshot.limits.as_ref()?.session.as_ref()?;
            Some(percent_label(window.percent_used))
        }
        TrayMetric::SessionAndWeek => None,
        TrayMetric::WeekPercent => {
            let window = snapshot.limits.as_ref()?.week_all.as_ref()?;
            Some(percent_label(window.percent_used))
        }
        TrayMetric::TodayCost => {
            let cost = snapshot.today.cost_usd;
            let text = if cost >= 1000.0 {
                format!("{:.1}k", cost / 1000.0)
            } else {
                format!("{}", cost.round() as i64)
            };
            // Cost has no ceiling to measure against, so it carries no level.
            Some((text, Level::Ok))
        }
        TrayMetric::TodayTokens => {
            let tokens = snapshot.today.tokens.output + snapshot.today.tokens.cache_read;
            let text = if tokens >= 1_000_000 {
                format!("{:.1}M", tokens as f64 / 1_000_000.0)
            } else {
                format!("{}k", tokens / 1000)
            };
            Some((text, Level::Ok))
        }
    }
}

fn percent_label(percent: f64) -> (String, Level) {
    (
        format!("{}", percent.round() as i64),
        Level::from_percent(percent),
    )
}

/// Forces the next update to redraw, even if the number is unchanged.
///
/// Switching which metric is shown can land on the same text, and the cache
/// would then keep the old icon.
pub fn forget_label() {
    *LAST_LABEL.lock().expect("tray label lock") = None;
}

/// Redraws the icon when the number changed.
///
/// The transcript loop publishes every few seconds; rasterizing and handing the
/// shell a new icon each time would be work nobody sees.
pub fn update(app: &tauri::AppHandle, snapshot: &Snapshot, settings: &Settings) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    // Two figures share one icon when configured that way; otherwise one.
    let rows = match stacked(snapshot, settings) {
        Some([top, bottom]) => vec![top, bottom],
        None => match label(snapshot, settings) {
            Some(single) => vec![single],
            None => return,
        },
    };
    let text = rows
        .iter()
        .map(|(figure, _)| figure.as_str())
        .collect::<Vec<_>>()
        .join("/");

    let mut last = LAST_LABEL.lock().expect("tray label lock");
    if last.as_deref() == Some(text.as_str()) {
        return;
    }
    *last = Some(text.clone());
    drop(last);

    // The bitmap exists only because Windows ignores set_title. macOS draws
    // that text beside the icon, so rasterizing the number there as well puts
    // it on screen twice — once in the level colour, once in the menu bar's.
    #[cfg(not(target_os = "macos"))]
    {
        let lines: Vec<(&str, Level)> = rows
            .iter()
            .map(|(figure, level)| (within_icon(figure), *level))
            .collect();
        let pixels = tray_icon::render_rows(&lines);
        let image = Image::new_owned(pixels, tray_icon::SIZE, tray_icon::SIZE);
        let _ = tray.set_icon(Some(image));
    }

    // The menu bar carries the number as text, so the icon beside it would be
    // one more thing to read. It is dropped here rather than never set: until
    // the first figure arrives this function returns early, and an item with
    // neither icon nor text is a gap nobody can click to reach the menu.
    #[cfg(target_os = "macos")]
    let _ = tray.set_icon(None);

    let _ = tray.set_tooltip(Some(tooltip(snapshot, settings.language)));

    // Linux and macOS show text beside the icon; Windows ignores this.
    let _ = tray.set_title(Some(&text));
}

fn tooltip(snapshot: &Snapshot, language: Language) -> String {
    let words = i18n::strings(language);
    let mut parts = vec![format!(
        "{} ${:.2} · {}",
        words.today, snapshot.today.cost_usd, words.at_api_rates
    )];
    if let Some(limits) = &snapshot.limits {
        if let Some(session) = &limits.session {
            parts.push(format!(
                "{} {}%",
                words.session,
                session.percent_used.round()
            ));
        }
        if let Some(week) = &limits.week_all {
            parts.push(format!("{} {}%", words.week, week.percent_used.round()));
        }
    }
    parts.join("\n")
}

/// Left click hides the widget if it is showing, and brings it back if not.
fn toggle_widget(app: &tauri::AppHandle) {
    let Some(win) = app.get_webview_window(WIDGET_LABEL) else {
        return;
    };
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Limits, Window};

    fn snapshot(session: Option<f64>, week: Option<f64>) -> Snapshot {
        Snapshot {
            limits: Some(Limits {
                session: session.map(|percent| Window {
                    percent_used: percent,
                    resets_at: 1,
                    forecast: None,
                }),
                week_all: week.map(|percent| Window {
                    percent_used: percent,
                    resets_at: 1,
                    forecast: None,
                }),
                week_model: None,
            }),
            ..Default::default()
        }
    }

    fn with(metric: TrayMetric) -> Settings {
        Settings {
            tray_metric: metric,
            ..Default::default()
        }
    }

    fn with_tokens(output: u64) -> Snapshot {
        let mut snapshot = Snapshot::default();
        snapshot.today.tokens.output = output;
        snapshot
    }

    #[test]
    fn a_figure_longer_than_the_icon_holds_survives_as_text() {
        let single = label(&with_tokens(12_300_000), &with(TrayMetric::TodayTokens));

        assert_eq!(single, Some(("12.3M".to_string(), Level::Ok)));
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn the_icon_takes_only_what_it_can_draw() {
        assert_eq!(within_icon("12.3M"), "12.");
        assert_eq!(within_icon("85"), "85");
        assert_eq!(within_icon("100"), "100");
    }

    #[test]
    fn the_stacked_metric_returns_both_figures_with_their_own_levels() {
        let pair = stacked(
            &snapshot(Some(85.0), Some(27.0)),
            &with(TrayMetric::SessionAndWeek),
        )
        .expect("pair");
        assert_eq!(pair[0], ("85".to_string(), Level::Danger));
        assert_eq!(pair[1], ("27".to_string(), Level::Ok));
    }

    /// Half a pair would be read as the wrong figure entirely.
    #[test]
    fn the_stacked_metric_needs_both_windows() {
        let settings = with(TrayMetric::SessionAndWeek);
        assert!(stacked(&snapshot(Some(85.0), None), &settings).is_none());
        assert!(stacked(&snapshot(None, Some(27.0)), &settings).is_none());
    }

    #[test]
    fn the_other_metrics_stay_single_line() {
        assert!(stacked(
            &snapshot(Some(85.0), Some(27.0)),
            &with(TrayMetric::SessionPercent)
        )
        .is_none());
        let single = label(
            &snapshot(Some(85.0), Some(27.0)),
            &with(TrayMetric::SessionPercent),
        );
        assert_eq!(single, Some(("85".to_string(), Level::Danger)));
    }

    /// The stacked metric has no single-line reading, so the caller must not
    /// fall back to one.
    #[test]
    fn the_stacked_metric_has_no_single_line_form() {
        assert!(label(
            &snapshot(Some(85.0), Some(27.0)),
            &with(TrayMetric::SessionAndWeek)
        )
        .is_none());
    }
}
