mod apps;
mod clean;
mod disk;
mod elevate;
mod hidden;
mod monitor;
mod state;
mod system;
mod updates;

use tauri::{Emitter, Manager};

use state::{AppState, BobEvent, Config};

pub use elevate::run as run_elevated;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    config: Config,
    version: String,
    update_ready: bool,
}

#[tauri::command]
fn state(app: tauri::AppHandle) -> Snapshot {
    let state = app.state::<AppState>();
    let shared = state.lock();
    Snapshot {
        config: shared.config.clone(),
        version: env!("CARGO_PKG_VERSION").into(),
        update_ready: shared.pending_installer.is_some(),
    }
}

#[tauri::command]
fn set_config(app: tauri::AppHandle, config: Config) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut shared = state.lock();
    shared.config = config;
    state::save_config(&shared);
    Ok(())
}

#[tauri::command]
fn set_window_material(window: tauri::WebviewWindow, dark: bool, glass: bool) {
    #[cfg(target_os = "windows")]
    {
        if glass {
            let tint = if dark { (16u8, 16, 18, 170) } else { (243, 241, 236, 176) };
            if window_vibrancy::apply_acrylic(&window, Some(tint)).is_err() {
                let _ = window_vibrancy::apply_mica(&window, Some(dark));
            }
        } else {
            let _ = window_vibrancy::clear_acrylic(&window);
            let _ = window_vibrancy::clear_mica(&window);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, dark, glass);
    }
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn setup_tray(app: &tauri::AppHandle) {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let Ok(open) = MenuItem::with_id(app, "open", "abrir", true, None::<&str>) else {
        return;
    };
    let Ok(quit) = MenuItem::with_id(app, "quit", "sair", true, None::<&str>) else {
        return;
    };
    let Ok(menu) = Menu::with_items(app, &[&open, &quit]) else {
        return;
    };
    let icon = tauri::include_image!("icons/32x32.png");
    let _ = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("bob tudo")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let minimize = window.state::<AppState>().lock().config.minimize_to_tray;
                if minimize {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            setup_tray(app.handle());
            monitor::start();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                updates::startup(&handle).await;
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
                    updates::periodic(&handle).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            state,
            set_config,
            set_window_material,
            disk::list_roots,
            disk::start_scan,
            disk::cancel_scan,
            disk::scan_children,
            disk::scan_biggest,
            disk::delete_to_trash,
            disk::open_in_explorer,
            clean::clean_preview,
            clean::clean_run,
            find_duplicates,
            system::startup_list,
            system::startup_set,
            system::privacy_list,
            system::privacy_set,
            system::disk_health,
            system::restore_list,
            system::restore_create,
            system::restore_undo,
            system::restore_system,
            monitor::monitor_sample,
            monitor::kill_process,
            apps::winget_search,
            apps::winget_install,
            apps::winget_upgrades,
            apps::winget_upgrade,
            apps::oss_catalog_cmd,
            apps::oss_search,
            apps::apps_list,
            apps::apps_uninstall,
            apps::apps_leftovers,
            apps::apps_remove_leftovers,
            updates::check_now,
            updates::apply_update
        ])
        .run(tauri::generate_context!())
        .expect("bob tudo");
}

#[tauri::command]
async fn find_duplicates(app: tauri::AppHandle, paths: Vec<String>, images: bool) -> Result<Vec<bob_core::DupeGroup>, String> {
    use std::sync::atomic::Ordering;
    if app.state::<AppState>().duping.swap(true, Ordering::SeqCst) {
        return Err("já existe uma busca".into());
    }
    let cancel = {
        let flag = app.state::<AppState>().lock().dupe_cancel.clone();
        flag.store(false, Ordering::SeqCst);
        flag
    };
    let progress = app.clone();
    let roots: Vec<std::path::PathBuf> = paths.into_iter().map(std::path::PathBuf::from).collect();
    let result = tokio::task::spawn_blocking(move || {
        bob_core::find_duplicates(&roots, images, &cancel, |step| {
            let _ = progress.emit(
                "bob",
                BobEvent::DupeProgress {
                    phase: step.phase,
                    done: step.done,
                    total: step.total,
                },
            );
        })
    })
    .await
    .map_err(|err| err.to_string());
    app.state::<AppState>().duping.store(false, Ordering::SeqCst);
    let groups = result??;
    app.state::<AppState>().lock().dupes = groups.clone();
    Ok(groups)
}
