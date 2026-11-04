mod apps;
mod catalog;
mod clean;
mod disk;
mod elevate;
mod hidden;
mod jobs;
mod managers;
mod monitor;
mod preload;
mod secrets;
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
    jobs: Vec<state::JobView>,
    disk: Option<bob_core::DiskSnapshot>,
    clean: Vec<clean::CleanCategory>,
    health: Vec<bob_core::DiskHealth>,
    startup: Vec<system::StartupItem>,
    upgrades: Vec<bob_core::WingetRow>,
    suggestions: Vec<bob_core::Suggestion>,
    updating: Vec<String>,
    managers: Vec<bob_core::ManagerInfo>,
}

#[tauri::command]
fn state(app: tauri::AppHandle) -> Snapshot {
    let state = app.state::<AppState>();
    let shared = state.lock();
    Snapshot {
        config: shared.config.clone(),
        version: env!("CARGO_PKG_VERSION").into(),
        update_ready: shared.pending_installer.is_some(),
        jobs: shared.jobs.clone(),
        disk: shared.disk.clone(),
        clean: shared.clean.clone(),
        health: shared.health.clone(),
        startup: shared.startup.clone(),
        upgrades: shared.upgrades.clone(),
        suggestions: shared.suggestions.clone(),
        updating: shared.updating.clone(),
        managers: shared.managers.clone(),
    }
}

#[tauri::command]
fn set_config(app: tauri::AppHandle, config: Config) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut shared = state.lock();
    shared.config = config.clone();
    state::save_config(&shared);
    drop(shared);
    sync_autostart(config.start_with_windows);
    Ok(())
}

fn sync_autostart(enabled: bool) {
    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let Ok((key, _)) = hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run") else {
            return;
        };
        if enabled {
            if let Ok(exe) = std::env::current_exe() {
                let _ = key.set_value("bob tudo", &format!("\"{}\" --tray", exe.display()));
            }
        } else {
            let _ = key.delete_value("bob tudo");
        }
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
    }
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
    let built = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("bob tudo")
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            if id == "open" {
                show_main(app);
            } else if id == "quit" {
                app.exit(0);
            } else if let Some(page) = id.strip_prefix("go:") {
                show_main(app);
                let _ = app.emit("bob", BobEvent::Navigate { page: page.into() });
            }
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
    if let Ok(icon) = built {
        jobs::store_tray(icon);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    bob_core::relax_process_keep_ui();
    let tray_launch = std::env::args().any(|arg| arg == "--tray");
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
        .setup(move |app| {
            sync_autostart(app.state::<AppState>().lock().config.start_with_windows);
            if let Some(window) = app.get_webview_window("main") {
                if tray_launch {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                }
            }
            setup_tray(app.handle());
            monitor::start();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                updates::startup(&handle).await;
            });
            preload::spawn(app.handle().clone());
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
            monitor::monitor_watch,
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
            updates::apply_update,
            managers::managers_list,
            managers::managers_refresh,
            managers::manager_install,
            managers::manager_update,
            catalog::catalog_search,
            catalog::catalog_detail,
            catalog::catalog_favorite,
            catalog::catalog_watch,
            catalog::catalog_collections,
            catalog::catalog_create_collection,
            catalog::catalog_collection_app,
            catalog::catalog_install,
            catalog::github_login_begin,
            catalog::github_login_poll,
            catalog::github_status,
            catalog::github_logout
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
    let job = jobs::begin(&app, "duplicados", "dupes", false);
    let job_id = job.clone();
    let hashes = app.state::<AppState>().lock().hash_store.clone();
    let index = app.state::<AppState>().lock().scan.clone();
    let data_dir = app.state::<AppState>().lock().data_dir.clone();
    let result = tokio::task::spawn_blocking(move || {
        bob_core::relax_thread();
        let mut hashes = hashes;
        let found = bob_core::find_duplicates_with(&roots, images, &cancel, |step| {
            let fraction = if step.total == 0 {
                -1.0
            } else {
                step.done as f32 / step.total as f32
            };
            jobs::tick(&progress, &job_id, fraction, &step.phase);
            let _ = progress.emit(
                "bob",
                BobEvent::DupeProgress {
                    phase: step.phase,
                    done: step.done,
                    total: step.total,
                },
            );
        }, index.as_ref(), &mut hashes)?;
        if hashes.prefix.len() > 80_000 {
            hashes.prefix.clear();
            hashes.full.clear();
        }
        state::write_json(&data_dir.join("hash-cache.json"), &hashes);
        Ok::<_, String>((found, hashes))
    })
    .await
    .map_err(|err| err.to_string());
    app.state::<AppState>().duping.store(false, Ordering::SeqCst);
    let (groups, hashes) = match result {
        Ok(Ok(pair)) => pair,
        Ok(Err(err)) => {
            jobs::finish(&app, &job, false, &err);
            return Err(err);
        }
        Err(err) => {
            jobs::finish(&app, &job, false, &err);
            return Err(err);
        }
    };
    app.state::<AppState>().lock().hash_store = hashes;
    jobs::finish(&app, &job, true, format!("{} grupos", groups.len()));
    app.state::<AppState>().lock().dupes = groups.clone();
    Ok(groups)
}
