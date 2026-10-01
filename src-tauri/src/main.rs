// Keeps the console window from appearing alongside the widget on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    claude_usage_widget_lib::run()
}
