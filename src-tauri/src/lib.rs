pub mod aggregate;
pub mod alerts;
pub mod collector;
pub mod forecast;
pub mod i18n;
pub mod pricing;
pub mod settings;
pub mod snapshot;
pub mod transcript;
pub mod tray;
pub mod tray_icon;
pub mod usage_cli;
pub mod window;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use alerts::Alerts;
use collector::Collector;
use settings::Settings;
use snapshot::Snapshot;
use tauri::{Emitter, Manager};

/// Backend owns the state; the frontend only renders what it is handed.
pub struct AppState {
    pub snapshot: Mutex<Snapshot>,
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    pub collector: Arc<Collector>,
    pub alerts: Mutex<Alerts>,
}

#[tauri::command]
fn get_snapshot(state: tauri::State<'_, AppState>) -> Snapshot {
    state.snapshot.lock().expect("snapshot lock").clone()
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Settings {
    state.settings.lock().expect("settings lock").clone()
}

/// Rebuilds the snapshot from both sources and pushes it to every window.
pub fn publish(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().expect("settings lock").clone();
    let next = state.collector.snapshot(&settings, collector::now());

    #[cfg(debug_assertions)]
    {
        let previous = state.snapshot.lock().expect("snapshot lock");
        if previous.today.cost_usd != next.today.cost_usd
            || previous.limits.is_some() != next.limits.is_some()
        {
            eprintln!(
                "[snapshot] today=${:.2} models={} limits={} error={:?}",
                next.today.cost_usd,
                next.by_model.len(),
                next.limits.is_some(),
                next.limits_error
            );
        }
    }

    for alert in state
        .alerts
        .lock()
        .expect("alerts lock")
        .check(&next, &settings)
    {
        use tauri_plugin_notification::NotificationExt;
        // The alert carries values; the wording is picked here, in whichever
        // language is configured.
        let words = i18n::strings(settings.language);
        let which = match alert.window {
            alerts::AlertWindow::Session => words.session,
            alerts::AlertWindow::Week => words.week,
        };
        let body = if alert.will_run_out {
            words.will_run_out
        } else {
            words.past_threshold
        };
        let _ = app
            .notification()
            .builder()
            .title(format!("{which} {}%", alert.percent.round()))
            .body(body)
            .show();
    }

    tray::update(app, &next, &settings);
    if let Some(win) = app.get_webview_window(window::WIDGET_LABEL) {
        window::update_taskbar(&win, &next, &settings);
    }
    *state.snapshot.lock().expect("snapshot lock") = next.clone();
    let _ = app.emit("snapshot", next);
}

/// Reads appended transcript bytes on a short cadence.
fn spawn_transcript_loop(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        let (interval, sources) = {
            let state = app.state::<AppState>();
            let interval = state
                .settings
                .lock()
                .expect("settings lock")
                .transcript_interval_seconds;
            (interval, Arc::clone(&state.collector))
        };

        sources.refresh_transcripts(collector::now());
        publish(&app);

        std::thread::sleep(Duration::from_secs(interval));
    });
}

/// Polls `claude -p "/usage"`. Kept apart from the transcript loop because one
/// poll costs about three seconds, and a slow or broken CLI must not hold up
/// the cost figures.
fn spawn_limits_loop(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        let (settings, sources) = {
            let state = app.state::<AppState>();
            let settings = state.settings.lock().expect("settings lock").clone();
            (settings, Arc::clone(&state.collector))
        };

        let wait = if settings.usage_enabled {
            sources.refresh_limits(collector::now())
        } else {
            // Off, so the command is never run; the loop only wakes to notice
            // the setting being turned back on.
            Duration::from_secs(settings.usage_interval_seconds)
        };
        publish(&app);

        std::thread::sleep(wait);
    });
}

pub const SETTINGS_LABEL: &str = "settings";

/// Pushes settings into the places that hold their own copy.
///
/// The polling loops read the settings afresh each pass, so intervals and the
/// usage toggle need nothing here; window properties and the backoff base do.
fn apply_settings(app: &tauri::AppHandle, settings: &Settings) {
    if let Some(win) = app.get_webview_window(window::WIDGET_LABEL) {
        let _ = win.set_always_on_top(settings.always_on_top);

        #[cfg(not(target_os = "macos"))]
        let _ = win.set_skip_taskbar(settings.skip_taskbar);
    }

    // Registering with the OS is a side effect outside the process, so it is
    // driven from the stored preference rather than assumed to match it.
    {
        use tauri_plugin_autostart::ManagerExt;
        let launcher = app.autolaunch();
        let enabled = launcher.is_enabled().unwrap_or(false);
        let outcome = if settings.autostart && !enabled {
            Some(launcher.enable())
        } else if !settings.autostart && enabled {
            Some(launcher.disable())
        } else {
            None
        };
        if let Some(Err(error)) = outcome {
            eprintln!("[autostart] {error}");
        }
    }

    tray::set_language(app, settings.language);
    if let Some(win) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = win.set_title(&format!(
            "Claude Usage — {}",
            i18n::strings(settings.language).settings_title
        ));
    }

    let state = app.state::<AppState>();
    let mut limits = state.collector.limits.lock().expect("limits lock");
    limits.set_interval(Duration::from_secs(settings.usage_interval_seconds));
    limits.set_claude_path(settings.claude_path.as_deref().map(PathBuf::from));
}

/// Stores the settings and returns what was actually stored.
///
/// The returned value is what the form redraws from, so a value the backend
/// clamped shows up in the window instead of silently differing from it.
#[tauri::command]
fn save_settings(app: tauri::AppHandle, next: Settings) -> Settings {
    let stored = next.sanitized();
    let state = app.state::<AppState>();

    *state.settings.lock().expect("settings lock") = stored.clone();
    if let Err(error) = stored.save(&state.settings_path) {
        eprintln!("[settings] could not save: {error}");
    }

    apply_settings(&app, &stored);
    // The widget holds its own copy to decide which cards to draw.
    let _ = app.emit("settings", stored.clone());
    tray::forget_label();
    publish(&app);
    stored
}

/// The tray menu reaches the same two actions the widget exposes.
pub fn open_settings_from_tray(app: &tauri::AppHandle) {
    open_settings(app.clone());
}

pub fn refresh_from_tray(app: &tauri::AppHandle) {
    retry_usage(app.clone());
}

fn settings_title(app: &tauri::AppHandle) -> String {
    let language = app
        .state::<AppState>()
        .settings
        .lock()
        .expect("settings lock")
        .language;
    format!("Claude Usage — {}", i18n::strings(language).settings_title)
}

#[tauri::command]
fn open_settings(app: tauri::AppHandle) {
    if let Some(existing) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = existing.show();
        let _ = existing.set_focus();
        return;
    }

    let built = tauri::WebviewWindowBuilder::new(
        &app,
        SETTINGS_LABEL,
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .title(settings_title(&app))
    .inner_size(360.0, 640.0)
    .resizable(true)
    .build();

    if let Err(error) = built {
        eprintln!("[settings] could not open window: {error}");
    }
}

/// Polls `/usage` now instead of waiting out the interval, which after
/// repeated failures has widened to as much as thirty minutes.
#[tauri::command]
fn retry_usage(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let sources = Arc::clone(&app.state::<AppState>().collector);
        sources.refresh_limits(collector::now());
        publish(&app);
    });
}

/// The widget sizes itself to whichever cards are switched on, so the window
/// follows the rendered height rather than a fixed one.
#[tauri::command]
fn resize_widget(app: tauri::AppHandle, width: f64, height: f64) {
    if let Some(win) = app.get_webview_window(window::WIDGET_LABEL) {
        window::resize_to_content(&win, width, height);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let position_writer = Arc::new(Mutex::new(window::PositionWriter::new()));
    let on_move = Arc::clone(&position_writer);

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_settings,
            resize_widget,
            retry_usage,
            save_settings,
            open_settings
        ])
        .on_window_event(move |win, event| match event {
            tauri::WindowEvent::Moved(position) => {
                if win.label() == window::WIDGET_LABEL {
                    on_move
                        .lock()
                        .expect("position lock")
                        .moved(win.app_handle(), *position);
                }
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // The app lives in the tray, so closing the widget hides it
                // rather than ending the process.
                if win.label() == window::WIDGET_LABEL {
                    api.prevent_close();
                    let _ = win.hide();
                }
            }
            tauri::WindowEvent::Destroyed => {
                on_move
                    .lock()
                    .expect("position lock")
                    .flush(win.app_handle());
            }
            _ => {}
        })
        .setup(|app| {
            let settings_path =
                settings::default_path().unwrap_or_else(|| PathBuf::from(settings::FILE_NAME));
            let settings = Settings::load(&settings_path);
            let collector = Arc::new(Collector::new(&settings));

            if let Some(win) = app.get_webview_window(window::WIDGET_LABEL) {
                window::restore(&win, &settings);
            }

            let language = settings.language;
            app.manage(AppState {
                snapshot: Mutex::new(Snapshot::default()),
                settings: Mutex::new(settings),
                settings_path,
                collector,
                alerts: Mutex::new(Alerts::new()),
            });

            tray::build(app.handle(), language)?;

            // Enforce the stored autostart preference, which lives in the OS
            // and can be changed from outside the app.
            let stored = app
                .state::<AppState>()
                .settings
                .lock()
                .expect("settings lock")
                .clone();
            apply_settings(app.handle(), &stored);

            spawn_transcript_loop(app.handle().clone());
            spawn_limits_loop(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running application");
}
