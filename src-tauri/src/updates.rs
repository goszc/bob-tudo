use std::time::Duration;

use base64::Engine;
use minisign_verify::{PublicKey, Signature};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

use crate::hidden::tokio_command;
use crate::state::{AppState, BobEvent};

const PUBKEY: &str = include_str!("../../packaging/updater.pub");
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const UPDATE_ENDPOINT: &str = "https://github.com/goszc/bob-tudo/releases/latest/download/latest.json";
const CHECK_EVERY: Duration = Duration::from_secs(3 * 60 * 60);

#[derive(Deserialize)]
struct Manifest {
    version: String,
    platforms: Platforms,
}

#[derive(Deserialize)]
struct Platforms {
    #[serde(rename = "windows-x86_64")]
    windows: Option<PlatformFile>,
}

#[derive(Deserialize)]
struct PlatformFile {
    signature: String,
    url: String,
}

pub async fn startup(app: &AppHandle) {
    let auto = app.state::<AppState>().lock().config.auto_update;
    if auto {
        let _ = check_app(app, false).await;
    }
}

pub async fn periodic(app: &AppHandle) {
    let state = app.state::<AppState>();
    let due = state.lock().config.auto_update;
    if due {
        let _ = check_app(app, false).await;
    }
    let _ = CHECK_EVERY;
}

#[tauri::command]
pub async fn check_now(app: AppHandle) -> Result<(), String> {
    let found = check_app(&app, true).await?;
    if !found {
        let _ = app.emit(
            "bob",
            BobEvent::Toast {
                message: "tudo atualizado".into(),
            },
        );
    }
    Ok(())
}

#[tauri::command]
pub async fn apply_update(app: AppHandle) -> Result<(), String> {
    let path = app
        .state::<AppState>()
        .lock()
        .pending_installer
        .clone()
        .ok_or_else(|| "sem instalador".to_string())?;
    tokio_command(&path).arg("/S").spawn().map_err(|err| err.to_string())?;
    app.exit(0);
    Ok(())
}

async fn check_app(app: &AppHandle, force: bool) -> Result<bool, String> {
    let endpoint = {
        let state = app.state::<AppState>();
        let custom = state.lock().config.update_url.trim().to_string();
        if !custom.is_empty() { custom } else { UPDATE_ENDPOINT.to_string() }
    };
    if endpoint.is_empty() {
        return Ok(false);
    }
    let client = reqwest::Client::builder()
        .user_agent("bob-tudo")
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| err.to_string())?;
    let response = match client.get(&endpoint).send().await {
        Ok(response) => response,
        Err(err) => {
            if force {
                return Err(err.to_string());
            }
            return Ok(false);
        }
    };
    if !response.status().is_success() {
        return Ok(false);
    }
    let manifest: Manifest = serde_json::from_slice(&response.bytes().await.map_err(|err| err.to_string())?).map_err(|err| err.to_string())?;
    if !bob_core::newer(&manifest.version, APP_VERSION) {
        return Ok(false);
    }
    let Some(file) = manifest.platforms.windows else {
        return Ok(false);
    };
    let bytes = client
        .get(&file.url)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?
        .bytes()
        .await
        .map_err(|err| err.to_string())?;
    verify(&bytes, &file.signature)?;
    let dir = app.state::<AppState>().lock().data_dir.join("updates");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("bob-tudo-setup.exe");
    let mut out = tokio::fs::File::create(&path).await.map_err(|err| err.to_string())?;
    out.write_all(&bytes).await.map_err(|err| err.to_string())?;
    app.state::<AppState>().lock().pending_installer = Some(path);
    let _ = app.emit(
        "bob",
        BobEvent::Update {
            version: manifest.version,
        },
    );
    Ok(true)
}

fn verify(bytes: &[u8], signature: &str) -> Result<(), String> {
    let key = PublicKey::decode(&minisign_text(PUBKEY)?).map_err(|err| err.to_string())?;
    let signature = Signature::decode(&minisign_text(signature)?).map_err(|err| err.to_string())?;
    key.verify(bytes, &signature, false).map_err(|err| err.to_string())
}

fn minisign_text(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.lines().nth(1).is_some() {
        return Ok(trimmed.to_string());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(trimmed)
        .map_err(|err| err.to_string())?;
    String::from_utf8(bytes).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_key_decodes() {
        let text = minisign_text(PUBKEY).expect("public key text");
        PublicKey::decode(&text).expect("public key");
    }
}
