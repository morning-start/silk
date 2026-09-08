use tauri::menu::MenuItem;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

use crate::application::state::AppState;

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(main) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn hide_main_window<R: Runtime>(window: &tauri::Window<R>) {
    let _ = window.hide();
}

fn handle_tray_action<R: Runtime>(app: &AppHandle<R>, action: &str) {
    match action {
        show => show_main_window(app),
        start_gateway => {
            let state = app.state::<AppState>().inner().clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::application::gateway_service::start(&state).await;
            });
        }
        stop_gateway => {
            let state = app.state::<AppState>().inner().clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::application::gateway_service::stop(&state).await;
            });
        }
        quit => app.exit(0),
        _ => {}
    }
}
