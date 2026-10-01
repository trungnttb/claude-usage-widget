//! Places and remembers the floating widget window.

use std::time::{Duration, Instant};

use tauri::window::{ProgressBarState, ProgressBarStatus};
use tauri::{LogicalSize, Manager, PhysicalPosition, WebviewWindow};

use crate::settings::Settings;
use crate::snapshot::Snapshot;
#[cfg(windows)]
use crate::tray_icon;
use crate::tray_icon::Level;
use crate::AppState;

pub const WIDGET_LABEL: &str = "widget";

/// Dragging emits a move event per frame; writing the file that often would
/// hammer the disk for a value only the next launch reads.
const SAVE_THROTTLE: Duration = Duration::from_secs(1);

/// Applies the stored window settings at startup.
pub fn restore(window: &WebviewWindow, settings: &Settings) {
    let _ = window.set_always_on_top(settings.always_on_top);

    #[cfg(not(target_os = "macos"))]
    let _ = window.set_skip_taskbar(settings.skip_taskbar);

    if let Some((x, y)) = settings.window_position {
        if is_on_a_screen(window, x, y) {
            let _ = window.set_position(PhysicalPosition::new(x, y));
        } else {
            // The monitor it used to sit on is gone, or the layout changed.
            // Restoring anyway would put the widget where nobody can reach it.
            let _ = window.center();
        }
    }
}

/// True when the window's top-left, plus enough of its edge to grab, falls
/// inside a monitor that currently exists.
fn is_on_a_screen(window: &WebviewWindow, x: i32, y: i32) -> bool {
    /// Enough of the window's own area to be draggable.
    const GRAB_MARGIN: i32 = 40;

    let Ok(monitors) = window.available_monitors() else {
        return false;
    };
    monitors.iter().any(|monitor| {
        let position = monitor.position();
        let size = monitor.size();
        let left = position.x;
        let top = position.y;
        let right = position.x + size.width as i32;
        let bottom = position.y + size.height as i32;

        x + GRAB_MARGIN >= left && x < right && y + GRAB_MARGIN >= top && y < bottom
    })
}

/// Remembers where the user put the window.
///
/// Called from the window event handler, which fires continuously while
/// dragging, so writes are throttled and the last position is flushed when the
/// window closes.
pub struct PositionWriter {
    last_write: Option<Instant>,
    pending: Option<(i32, i32)>,
}

impl PositionWriter {
    pub fn new() -> Self {
        Self {
            last_write: None,
            pending: None,
        }
    }

    pub fn moved(&mut self, app: &tauri::AppHandle, position: PhysicalPosition<i32>) {
        self.pending = Some((position.x, position.y));
        let due = self
            .last_write
            .is_none_or(|last| last.elapsed() >= SAVE_THROTTLE);
        if due {
            self.flush(app);
        }
    }

    pub fn flush(&mut self, app: &tauri::AppHandle) {
        let Some(position) = self.pending.take() else {
            return;
        };
        self.last_write = Some(Instant::now());

        let state = app.state::<AppState>();
        let path = state.settings_path.clone();
        let settings = {
            let mut settings = state.settings.lock().expect("settings lock");
            settings.window_position = Some(position);
            settings.clone()
        };
        if let Err(error) = settings.save(&path) {
            eprintln!("[window] could not save position: {error}");
        }
    }
}

impl Default for PositionWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// Shows the session percentage on the taskbar button, or the Dock icon.
///
/// Skipped entirely when the window is hidden from the taskbar: there is
/// nothing for the indicator to appear on, so the calls would be waste.
pub fn update_taskbar(window: &WebviewWindow, snapshot: &Snapshot, settings: &Settings) {
    if settings.skip_taskbar {
        let _ = window.set_progress_bar(ProgressBarState {
            status: Some(ProgressBarStatus::None),
            progress: None,
        });
        return;
    }

    let percent = snapshot
        .limits
        .as_ref()
        .and_then(|limits| limits.session.as_ref())
        .map(|session| session.percent_used);

    let Some(percent) = percent else {
        let _ = window.set_progress_bar(ProgressBarState {
            status: Some(ProgressBarStatus::None),
            progress: None,
        });
        return;
    };

    let level = Level::from_percent(percent);
    let _ = window.set_progress_bar(ProgressBarState {
        // Windows paints these three states green, amber and red, which is the
        // same scale the widget and the tray icon use.
        status: Some(match level {
            Level::Ok => ProgressBarStatus::Normal,
            Level::Warn => ProgressBarStatus::Paused,
            Level::Danger => ProgressBarStatus::Error,
        }),
        progress: Some(percent.round().clamp(0.0, 100.0) as u64),
    });

    #[cfg(windows)]
    {
        let badge = tray_icon::render_badge(level);
        let image =
            tauri::image::Image::new_owned(badge, tray_icon::BADGE_SIZE, tray_icon::BADGE_SIZE);
        let _ = window.set_overlay_icon(Some(image));
    }

    // set_badge_count is unsupported on Windows, which uses the overlay above.
    #[cfg(not(windows))]
    {
        let _ = window.set_badge_count(Some(percent.round() as i64));
    }
}

/// Sizes the widget to its content so a hidden card leaves no empty space.
pub fn resize_to_content(window: &WebviewWindow, width: f64, height: f64) {
    let _ = window.set_size(LogicalSize::new(width, height.max(60.0)));
}
