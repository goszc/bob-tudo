use std::sync::Mutex;
use std::time::Instant;

use tauri::Emitter;
use tauri::Manager;

use crate::state::{AppState, BobEvent, JobView};

static TRAY: Mutex<Option<tauri::tray::TrayIcon>> = Mutex::new(None);
static TICKS: Mutex<Vec<(String, Instant)>> = Mutex::new(Vec::new());

pub fn store_tray(icon: tauri::tray::TrayIcon) {
    *TRAY.lock().unwrap_or_else(|err| err.into_inner()) = Some(icon);
}

pub fn begin(app: &tauri::AppHandle, title: &str, page: &str, quiet: bool) -> String {
    let id = bob_core::nid("j");
    let job = JobView {
        id: id.clone(),
        title: title.into(),
        page: page.into(),
        progress: -1.0,
        detail: "na fila".into(),
        state: "running".into(),
        quiet,
    };
    app.state::<AppState>().lock().jobs.insert(0, job);
    publish(app, true);
    id
}

pub fn tick(app: &tauri::AppHandle, id: &str, progress: f32, detail: &str) {
    {
        let mut ticks = TICKS.lock().unwrap_or_else(|err| err.into_inner());
        if let Some(slot) = ticks.iter_mut().find(|(key, _)| key == id) {
            if slot.1.elapsed().as_millis() < 280 {
                let state = app.state::<AppState>();
                let mut shared = state.lock();
                if let Some(job) = shared.jobs.iter_mut().find(|job| job.id == id && job.state == "running") {
                    job.progress = progress;
                    job.detail = detail.into();
                }
                return;
            }
            slot.1 = Instant::now();
        } else {
            ticks.push((id.to_string(), Instant::now()));
        }
    }
    let state = app.state::<AppState>();
    let mut shared = state.lock();
    if let Some(job) = shared.jobs.iter_mut().find(|job| job.id == id && job.state == "running") {
        job.progress = progress;
        job.detail = detail.into();
    }
    drop(shared);
    publish(app, false);
}

pub fn finish(app: &tauri::AppHandle, id: &str, ok: bool, detail: impl AsRef<str>) {
    let detail = detail.as_ref();
    let (title, quiet, page) = {
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        let Some(job) = shared.jobs.iter_mut().find(|job| job.id == id) else {
            return;
        };
        job.state = if ok { "done" } else { "error" }.into();
        job.progress = if ok { 1.0 } else { job.progress };
        job.detail = detail.into();
        (job.title.clone(), job.quiet, job.page.clone())
    };
    publish(app, true);
    if ok && !quiet {
        crate::system::notify_kind(app, "task", &format!("{title} terminou"));
        let _ = page;
    }
}

pub fn publish(app: &tauri::AppHandle, tray: bool) {
    let jobs = app.state::<AppState>().lock().jobs.clone();
    let _ = app.emit("bob", BobEvent::Jobs { jobs: jobs.clone() });
    if tray {
        refresh_tray(app, &jobs);
    }
}

fn refresh_tray(app: &tauri::AppHandle, jobs: &[JobView]) {
    use tauri::menu::{Menu, MenuItem};
    let Some(icon) = TRAY.lock().unwrap_or_else(|err| err.into_inner()).clone() else {
        return;
    };
    let Ok(open) = MenuItem::with_id(app, "open", "abrir", true, None::<&str>) else {
        return;
    };
    let Ok(quit) = MenuItem::with_id(app, "quit", "sair", true, None::<&str>) else {
        return;
    };
    let mut entries: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = vec![&open];
    let mut owned = Vec::new();
    for job in jobs.iter().filter(|job| job.state == "running").take(5) {
        let label = format!("{} · {}", job.title, job.detail);
        let id = format!("go:{}", job.page);
        if let Ok(item) = MenuItem::with_id(app, id, label, true, None::<&str>) {
            owned.push(item);
        }
    }
    for item in &owned {
        entries.push(item);
    }
    entries.push(&quit);
    if let Ok(menu) = Menu::with_items(app, &entries) {
        let _ = icon.set_menu(Some(menu));
    }
}

pub fn mark_updating(app: &tauri::AppHandle, area: &str, on: bool) {
    let state = app.state::<AppState>();
    let mut shared = state.lock();
    if on {
        if !shared.updating.iter().any(|item| item == area) {
            shared.updating.push(area.into());
        }
    } else {
        shared.updating.retain(|item| item != area);
    }
    drop(shared);
    let _ = app.emit(
        "bob",
        BobEvent::Updating {
            area: area.into(),
            on,
        },
    );
}
