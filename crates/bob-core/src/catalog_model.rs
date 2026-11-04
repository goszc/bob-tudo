//! Modelo do catálogo: juntar fontes, ler manifests e limpar o readme.
//! Nada aqui baixa a rede nem abre uma janela.

use serde::{Deserialize, Serialize};

use crate::catalog::oss_catalog;
use crate::version::newer;

pub const APP_INSTALLER_STORE: &str = "ms-windows-store://pdp/?ProductId=9NBLGGH4NNS1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub source: String,
    pub source_id: String,
    pub version: String,
    pub size: u64,
    pub portable: bool,
    pub installer: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Incoming {
    pub name: String,
    pub description: String,
    pub category: String,
    pub license: String,
    pub homepage: String,
    pub repo: String,
    pub winget_id: String,
    pub stars: u64,
    pub updated_ms: i64,
    pub open_source: bool,
    pub alternative_to: String,
    pub icon: String,
    pub source: String,
    pub source_id: String,
    pub version: String,
    pub size: u64,
    pub portable: bool,
    pub installer: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogHit {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub license: String,
    pub homepage: String,
    pub repo: String,
    pub winget_id: String,
    pub stars: u64,
    pub updated_ms: i64,
    pub latest_version: String,
    pub size: u64,
    pub icon: String,
    pub open_source: bool,
    pub portable: bool,
    pub installer: bool,
    pub alternative_to: String,
    pub sources: Vec<String>,
    pub offers: Vec<Offer>,
    pub installed: bool,
    pub favorite: bool,
    pub watched: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub url: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub published_ms: i64,
    pub assets: Vec<Asset>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDetail {
    #[serde(flatten)]
    pub hit: CatalogHit,
    pub readme_html: String,
    pub images: Vec<String>,
    pub releases: Vec<Release>,
    pub alternatives: Vec<CatalogHit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DevicePoll {
    Pending,
    SlowDown,
    Granted { token: String },
    Denied(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagerInfo {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub version: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

pub fn curated_incoming() -> Vec<Incoming> {
    oss_catalog()
        .into_iter()
        .map(|app| {
            let repo = app.repo.trim().trim_matches('/').to_string();
            let source_id = norm_name(&app.name);
            Incoming {
                name: app.name,
                description: app.description,
                category: if app.category.is_empty() { "outros".into() } else { app.category },
                license: String::new(),
                homepage: if repo.is_empty() { String::new() } else { format!("https://github.com/{repo}") },
                repo,
                winget_id: app.winget_id,
                stars: app.stars,
                updated_ms: 0,
                open_source: true,
                alternative_to: app.alternative_to,
                icon: String::new(),
                source: "curadoria".into(),
                source_id,
                version: String::new(),
                size: 0,
                portable: false,
                installer: false,
            }
        })
        .collect()
}

pub fn norm_name(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub fn norm_id(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub fn norm_repo(value: &str) -> String {
    github_repo(value).unwrap_or_else(|| {
        value
            .trim()
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("github.com/")
            .to_ascii_lowercase()
    })
}

pub fn github_repo(value: &str) -> Option<String> {
    let lower = value.trim().trim_end_matches('/').to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://github.com/")
        .or_else(|| lower.strip_prefix("http://github.com/"))
        .or_else(|| lower.strip_prefix("github.com/"))?;
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let owner = parts.next()?.trim_end_matches(".git");
    let repo = parts.next()?.trim_end_matches(".git");
    if owner.is_empty() || repo.is_empty() || repo.contains(' ') {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

pub fn merge(items: Vec<Incoming>) -> Vec<CatalogHit> {
    let mut items: Vec<Incoming> = items
        .into_iter()
        .filter(|item| !item.name.trim().is_empty() || !item.winget_id.trim().is_empty())
        .collect();
    for item in &mut items {
        if item.name.trim().is_empty() {
            item.name = item.source_id.clone();
        }
        if item.repo.trim().is_empty() {
            if let Some(repo) = github_repo(&item.homepage) {
                item.repo = repo;
            }
        }
        item.name = clean_text(&item.name, 200);
        item.description = clean_text(&item.description, 2000);
    }
    let n = items.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut by_wg = std::collections::HashMap::<String, usize>::new();
    let mut by_repo = std::collections::HashMap::<String, usize>::new();
    let mut by_name = std::collections::HashMap::<String, usize>::new();
    for i in 0..n {
        let wg = norm_id(&items[i].winget_id);
        if !wg.is_empty() {
            if let Some(prev) = by_wg.get(&wg).copied() {
                unite(&mut parent, i, prev);
            }
            by_wg.insert(wg, i);
        }
        let repo = norm_repo(&items[i].repo);
        if repo.contains('/') {
            if let Some(prev) = by_repo.get(&repo).copied() {
                unite(&mut parent, i, prev);
            }
            by_repo.insert(repo, i);
        }
    }
    for i in 0..n {
        let name = norm_name(&items[i].name);
        if name.len() < 3 {
            continue;
        }
        if let Some(prev) = by_name.get(&name).copied() {
            if compatible(&items[i], &items[prev]) {
                unite(&mut parent, i, prev);
            }
        } else {
            by_name.insert(name, i);
        }
    }
    let mut groups: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for i in 0..n {
        groups.entry(find(&mut parent, i)).or_default().push(i);
    }
    let mut hits: Vec<CatalogHit> = groups.into_values().map(|indexes| collapse(&items, &indexes)).collect();
    hits.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
    hits
}

fn compatible(a: &Incoming, b: &Incoming) -> bool {
    let aw = norm_id(&a.winget_id);
    let bw = norm_id(&b.winget_id);
    if !aw.is_empty() && !bw.is_empty() && aw != bw {
        return false;
    }
    let ar = norm_repo(&a.repo);
    let br = norm_repo(&b.repo);
    if ar.contains('/') && br.contains('/') && ar != br {
        return false;
    }
    true
}

fn collapse(items: &[Incoming], indexes: &[usize]) -> CatalogHit {
    let mut pick: Vec<&Incoming> = indexes.iter().map(|i| &items[*i]).collect();
    pick.sort_by_key(|item| source_rank(&item.source));
    let mut hit = CatalogHit {
        id: String::new(),
        name: String::new(),
        description: String::new(),
        category: "outros".into(),
        license: String::new(),
        homepage: String::new(),
        repo: String::new(),
        winget_id: String::new(),
        stars: 0,
        updated_ms: 0,
        latest_version: String::new(),
        size: 0,
        icon: String::new(),
        open_source: false,
        portable: false,
        installer: false,
        alternative_to: String::new(),
        sources: Vec::new(),
        offers: Vec::new(),
        installed: false,
        favorite: false,
        watched: false,
    };
    for item in &pick {
        if hit.name.is_empty() || item.source == "curadoria" {
            hit.name = item.name.clone();
        }
        if (hit.description.is_empty() || item.source == "curadoria") && !item.description.is_empty() {
            hit.description = item.description.clone();
        }
        if (hit.category == "outros" || item.source == "curadoria") && !item.category.is_empty() && item.category != "outros" {
            hit.category = item.category.clone();
        }
        if hit.license.is_empty() && !item.license.is_empty() {
            hit.license = item.license.clone();
        }
        if hit.homepage.is_empty() && !item.homepage.is_empty() {
            hit.homepage = item.homepage.clone();
        }
        if hit.repo.is_empty() && norm_repo(&item.repo).contains('/') {
            hit.repo = norm_repo(&item.repo);
        }
        if hit.winget_id.is_empty() && !item.winget_id.is_empty() {
            hit.winget_id = item.winget_id.clone();
        }
        if item.source == "winget" && !item.source_id.is_empty() {
            hit.winget_id = item.source_id.clone();
        }
        hit.stars = hit.stars.max(item.stars);
        hit.updated_ms = hit.updated_ms.max(item.updated_ms);
        hit.open_source |= item.open_source;
        if hit.alternative_to.is_empty() && !item.alternative_to.is_empty() {
            hit.alternative_to = item.alternative_to.clone();
        }
        if hit.icon.is_empty() && !item.icon.is_empty() {
            hit.icon = item.icon.clone();
        }
        if !item.version.is_empty() && (hit.latest_version.is_empty() || newer(&item.version, &hit.latest_version)) {
            hit.latest_version = item.version.clone();
        }
        hit.size = hit.size.max(item.size);
        if item.source != "curadoria" {
            let offer = Offer {
                source: item.source.clone(),
                source_id: if item.source_id.is_empty() { item.winget_id.clone() } else { item.source_id.clone() },
                version: item.version.clone(),
                size: item.size,
                portable: item.portable,
                installer: item.installer,
            };
            if !hit.offers.iter().any(|have| have.source == offer.source && have.source_id == offer.source_id) {
                hit.offers.push(offer);
            }
        }
    }
    hit.portable = hit.offers.iter().any(|offer| offer.portable);
    hit.installer = hit.offers.iter().any(|offer| offer.installer);
    let mut sources: Vec<String> = hit.offers.iter().map(|offer| offer.source.clone()).collect();
    if hit.repo.contains('/') && !sources.iter().any(|source| source == "github") {
        sources.push("github".into());
    }
    sources.sort();
    sources.dedup();
    hit.sources = sources;
    hit.id = if !hit.winget_id.is_empty() {
        format!("wg:{}", norm_id(&hit.winget_id))
    } else if hit.repo.contains('/') {
        format!("gh:{}", hit.repo)
    } else {
        format!("name:{}", norm_name(&hit.name))
    };
    if hit.name.is_empty() {
        hit.name = hit.id.clone();
    }
    hit
}

fn source_rank(source: &str) -> u8 {
    match source {
        "curadoria" => 0,
        "github" => 1,
        "scoop" => 2,
        "winget" => 3,
        "chocolatey" => 4,
        _ => 5,
    }
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

fn unite(parent: &mut [usize], a: usize, b: usize) {
    let a = find(parent, a);
    let b = find(parent, b);
    if a != b {
        parent[b] = a;
    }
}

fn clean_text(value: &str, max: usize) -> String {
    value.chars().filter(|c| *c != '\0').take(max).collect::<String>().trim().to_string()
}

pub fn parse_scoop(file_stem: &str, json: &str) -> Option<Incoming> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    if value.get("version").is_none() && value.get("url").is_none() && value.get("architecture").is_none() {
        return None;
    }
    let name = value.get("##").and_then(|v| v.as_str()).unwrap_or(file_stem).trim();
    let name = if name.is_empty() { file_stem } else { name };
    let homepage = value.get("homepage").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let license = match value.get("license") {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Object(map)) => map.get("identifier").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        _ => String::new(),
    };
    let url = value
        .pointer("/architecture/64bit/url")
        .or_else(|| value.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let size = value
        .pointer("/architecture/64bit/size")
        .or_else(|| value.get("size"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Some(Incoming {
        name: name.into(),
        description: value.get("description").and_then(|v| v.as_str()).unwrap_or("").into(),
        category: "outros".into(),
        license,
        homepage: homepage.clone(),
        repo: github_repo(&homepage).unwrap_or_default(),
        winget_id: String::new(),
        stars: 0,
        updated_ms: 0,
        open_source: true,
        alternative_to: String::new(),
        icon: String::new(),
        source: "scoop".into(),
        source_id: file_stem.to_string(),
        version: value.get("version").and_then(|v| v.as_str()).unwrap_or("").into(),
        size,
        portable: true,
        installer: url.ends_with(".msi") || url.ends_with(".exe"),
    })
}

pub fn parse_choco_atom(xml: &str) -> Vec<Incoming> {
    let mut out = Vec::new();
    for block in xml.split("<entry").skip(1) {
        let block = match block.split_once("</entry>") {
            Some((body, _)) => body,
            None => block,
        };
        let latest = tag_text(block, "IsLatestVersion");
        if latest.eq_ignore_ascii_case("false") {
            continue;
        }
        let id = tag_text(block, "Id");
        if id.is_empty() {
            continue;
        }
        let title = {
            let title = tag_text(block, "Title");
            if title.is_empty() { id.clone() } else { title }
        };
        let license_url = tag_text(block, "LicenseUrl");
        let tags = tag_text(block, "Tags");
        let homepage = tag_text(block, "ProjectUrl");
        let license = if license_url.is_empty() { String::new() } else { license_url };
        out.push(Incoming {
            name: title,
            description: tag_text(block, "Description"),
            category: category_from_tags(&tags),
            license: license.clone(),
            homepage: homepage.clone(),
            repo: github_repo(&homepage).unwrap_or_default(),
            winget_id: String::new(),
            stars: 0,
            updated_ms: rfc3339_ms(&tag_text(block, "Published")),
            open_source: looks_oss(&license, &tags),
            alternative_to: String::new(),
            icon: tag_text(block, "IconUrl"),
            source: "chocolatey".into(),
            source_id: id,
            version: tag_text(block, "Version"),
            size: tag_text(block, "PackageSize").parse().unwrap_or(0),
            portable: false,
            installer: true,
        });
    }
    out
}

fn tag_text(block: &str, name: &str) -> String {
    let markers = [format!("<d:{name}"), format!("<{name}")];
    for marker in markers {
        let Some(start) = block.find(&marker) else { continue };
        let after = &block[start + marker.len()..];
        let Some(close) = after.find('>') else { continue };
        let rest = &after[close + 1..];
        let end_tag = format!("</");
        let Some(end) = rest.find(&end_tag) else { continue };
        return decode_xml(rest[..end].trim());
    }
    String::new()
}

fn decode_xml(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn category_from_tags(tags: &str) -> String {
    let tags = tags.to_ascii_lowercase();
    const MAP: &[(&str, &str)] = &[
        ("browser", "navegador"),
        ("navegador", "navegador"),
        ("video", "vídeo"),
        ("audio", "áudio"),
        ("player", "vídeo"),
        ("image", "imagem"),
        ("photo", "imagem"),
        ("office", "escritório"),
        ("pdf", "escritório"),
        ("security", "segurança"),
        ("password", "segurança"),
        ("developer", "desenvolvimento"),
        ("git", "desenvolvimento"),
        ("chat", "comunicação"),
        ("mail", "comunicação"),
    ];
    for (needle, category) in MAP {
        if tags.contains(needle) {
            return (*category).into();
        }
    }
    "outros".into()
}

fn looks_oss(license: &str, tags: &str) -> bool {
    let text = format!("{license} {tags}").to_ascii_lowercase();
    ["mit", "apache", "gpl", "bsd", "mpl", "isc", "unlicense", "zlib", "lgpl", "artistic", "opensource", "open-source", "foss"]
        .iter()
        .any(|key| text.contains(key))
}

pub fn parse_github_releases(json: &str) -> Vec<Release> {
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let list = value.as_array().cloned().unwrap_or_else(|| vec![value]);
    let mut releases = Vec::new();
    for item in list {
        if item.get("draft").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        let version = item.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        if version.is_empty() {
            continue;
        }
        let mut assets = Vec::new();
        if let Some(list) = item.get("assets").and_then(|v| v.as_array()) {
            for asset in list {
                let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let url = asset.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if name.is_empty() || url.is_empty() || !windows_asset(&name) {
                    continue;
                }
                assets.push(Asset {
                    kind: asset_kind(&name),
                    size: asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
                    name,
                    url,
                });
            }
        }
        releases.push(Release {
            version,
            notes: clean_text(item.get("body").and_then(|v| v.as_str()).unwrap_or(""), 8000),
            url: item.get("html_url").and_then(|v| v.as_str()).unwrap_or("").into(),
            published_ms: rfc3339_ms(item.get("published_at").and_then(|v| v.as_str()).unwrap_or("")),
            assets,
        });
    }
    releases
}

fn windows_asset(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if name.ends_with(".sha256") || name.ends_with(".sig") || name.ends_with(".asc") {
        return false;
    }
    const SKIP: &[&str] = &["linux", "darwin", "macos", ".dmg", ".deb", ".rpm", ".appimage", "freebsd"];
    if SKIP.iter().any(|needle| name.contains(needle)) {
        return false;
    }
    name.ends_with(".exe")
        || name.ends_with(".msi")
        || name.ends_with(".msix")
        || name.ends_with(".msixbundle")
        || name.ends_with(".zip")
        || name.contains("windows")
        || name.contains("win64")
        || name.contains("win32")
}

fn asset_kind(name: &str) -> String {
    let name = name.to_ascii_lowercase();
    if name.ends_with(".exe") {
        "exe".into()
    } else if name.ends_with(".msi") || name.ends_with(".msix") || name.ends_with(".msixbundle") {
        "msi".into()
    } else if name.ends_with(".zip") {
        "zip".into()
    } else {
        "outro".into()
    }
}

pub fn parse_github_repo(json: &str) -> Option<(u64, String, i64, String)> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let stars = value.get("stargazers_count")?.as_u64()?;
    let description = value.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let pushed = rfc3339_ms(value.get("pushed_at").and_then(|v| v.as_str()).unwrap_or(""));
    let license = value.pointer("/license/spdx_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    Some((stars, description, pushed, license))
}

pub fn winget_installer_assets(release_json: &str) -> Vec<RemoteAsset> {
    let value: serde_json::Value = match serde_json::from_str(release_json) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let Some(list) = value.get("assets").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for asset in list {
        let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let url = asset.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() || url.is_empty() {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        let keep = lower.ends_with(".msixbundle")
            || lower.ends_with(".msix")
            || (lower.contains("depend") && lower.ends_with(".zip"));
        if keep {
            out.push(RemoteAsset {
                name: name.into(),
                url: url.into(),
                size: asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
            });
        }
    }
    out
}

pub fn parse_device_start(body: &str) -> Result<DeviceStart, String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|err| err.to_string())?;
    if let Some(err) = value.get("error").and_then(|v| v.as_str()) {
        return Err(value.get("error_description").and_then(|v| v.as_str()).unwrap_or(err).into());
    }
    Ok(DeviceStart {
        device_code: value.get("device_code").and_then(|v| v.as_str()).unwrap_or("").into(),
        user_code: value.get("user_code").and_then(|v| v.as_str()).unwrap_or("").into(),
        verification_uri: value
            .get("verification_uri")
            .and_then(|v| v.as_str())
            .unwrap_or("https://github.com/login/device")
            .into(),
        interval: value.get("interval").and_then(|v| v.as_u64()).unwrap_or(5).max(3),
    })
}

pub fn parse_device_poll(body: &str) -> DevicePoll {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return DevicePoll::Denied("resposta ilegível".into());
    };
    if let Some(token) = value.get("access_token").and_then(|v| v.as_str()) {
        if !token.is_empty() {
            return DevicePoll::Granted { token: token.into() };
        }
    }
    match value.get("error").and_then(|v| v.as_str()).unwrap_or("") {
        "authorization_pending" => DevicePoll::Pending,
        "slow_down" => DevicePoll::SlowDown,
        "" => DevicePoll::Denied("sem token".into()),
        other => DevicePoll::Denied(value.get("error_description").and_then(|v| v.as_str()).unwrap_or(other).into()),
    }
}

pub fn render_readme(markdown: &str, raw_base: &str) -> (String, Vec<String>) {
    let images = markdown_images(markdown, raw_base);
    let rewritten = rewrite_markdown(markdown, raw_base);
    let mut html = String::new();
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    let parser = pulldown_cmark::Parser::new_ext(&rewritten, options);
    pulldown_cmark::html::push_html(&mut html, parser);
    let clean = ammonia::Builder::default()
        .url_relative(ammonia::UrlRelative::Deny)
        .link_rel(Some("noopener noreferrer"))
        .add_tag_attributes("img", &["src", "alt", "title"])
        .clean(&html)
        .to_string();
    (clean, images)
}

fn markdown_images(markdown: &str, raw_base: &str) -> Vec<String> {
    let mut images = Vec::new();
    let bytes = markdown.as_bytes();
    let mut i = 0;
    while i + 4 < bytes.len() {
        if bytes[i] == b'!' && bytes.get(i + 1) == Some(&b'[') {
            if let Some(end) = markdown[i..].find("](") {
                let rest = &markdown[i + end + 2..];
                if let Some(close) = rest.find(')') {
                    let url = absolutize(rest[..close].trim().trim_matches(['<', '>']), raw_base);
                    if is_image(&url) {
                        images.push(url);
                    }
                    i += end + 2 + close;
                    continue;
                }
            }
        }
        i += 1;
    }
    images
}

fn is_image(url: &str) -> bool {
    let lower = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"].iter().any(|ext| lower.ends_with(ext))
}

fn rewrite_markdown(markdown: &str, raw_base: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let chars: Vec<char> = markdown.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == ']' && chars.get(i + 1) == Some(&'(') {
            out.push(']');
            out.push('(');
            i += 2;
            let start = i;
            while i < chars.len() && chars[i] != ')' {
                i += 1;
            }
            let url: String = chars[start..i].iter().collect();
            out.push_str(&absolutize(url.trim(), raw_base));
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn absolutize(url: &str, raw_base: &str) -> String {
    let url = url.trim().trim_matches(['<', '>', '"', '\'']);
    if url.is_empty() || url.starts_with('#') || url.starts_with("mailto:") {
        return url.to_string();
    }
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        return url.to_string();
    }
    if lower.starts_with("javascript:") || lower.starts_with("data:") {
        return String::new();
    }
    if raw_base.is_empty() {
        return String::new();
    }
    let base = raw_base.trim_end_matches('/');
    if let Some(rest) = url.strip_prefix('/') {
        if let Some(idx) = base.find("/HEAD") {
            return format!("{}/{rest}", &base[..idx + "/HEAD".len()]);
        }
        return format!("{base}/{rest}");
    }
    format!("{base}/{url}")
}

pub fn rfc3339_ms(value: &str) -> i64 {
    let value = value.trim();
    if value.len() < 10 {
        return 0;
    }
    let Ok(y) = value[0..4].parse::<i64>() else { return 0 };
    let Ok(m) = value[5..7].parse::<i64>() else { return 0 };
    let Ok(d) = value[8..10].parse::<i64>() else { return 0 };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return 0;
    }
    let mut secs = days_from_civil(y, m, d).saturating_mul(86400);
    if value.len() >= 19 {
        let hh: i64 = value.get(11..13).unwrap_or("0").parse().unwrap_or(0);
        let mm: i64 = value.get(14..16).unwrap_or("0").parse().unwrap_or(0);
        let ss: i64 = value.get(17..19).unwrap_or("0").parse().unwrap_or(0);
        secs += hh.saturating_mul(3600) + mm.saturating_mul(60) + ss;
    }
    secs.saturating_mul(1000)
}

fn days_from_civil(mut y: i64, m: i64, d: i64) -> i64 {
    y -= if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn fts_query(text: &str) -> String {
    text.split_whitespace()
        .map(|word| word.replace('"', "").replace('*', ""))
        .filter(|word| !word.is_empty())
        .map(|word| format!("\"{word}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn paid_label(alternative_to: &str) -> String {
    let lower = alternative_to.to_ascii_lowercase();
    let marker = "alternative ao ";
    lower
        .find(marker)
        .map(|index| alternative_to[index + marker.len()..].trim().to_string())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| alternative_to.trim().to_string())
}

pub fn preset_collections() -> Vec<(&'static str, &'static str, &'static [&'static str])> {
    vec![
        ("pc-novo", "pc novo", &["firefox", "vlc", "7-zip", "powertoys", "sumatra pdf", "sharex"]),
        ("dev", "dev", &["git", "vscodium", "windows terminal", "neovim", "dbeaver"]),
        ("midia", "mídia", &["vlc", "obs studio", "gimp", "audacity", "handbrake", "krita"]),
    ]
}

pub fn scoop_install_script() -> &'static str {
    "Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser -Force; Invoke-RestMethod -Uri https://get.scoop.sh | Invoke-Expression"
}

pub fn chocolatey_install_script() -> &'static str {
    "Set-ExecutionPolicy Bypass -Scope Process -Force; [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))"
}

pub fn manager_from_probe(id: &str, path: &str, output: &str) -> ManagerInfo {
    let installed = !path.trim().is_empty();
    ManagerInfo {
        name: manager_name(id).into(),
        version: if installed { version_from_output(id, output) } else { String::new() },
        path: if installed { path.trim().into() } else { String::new() },
        id: id.into(),
        installed,
    }
}

pub fn manager_name(id: &str) -> &'static str {
    match id {
        "winget" => "winget",
        "scoop" => "scoop",
        "chocolatey" => "chocolatey",
        _ => "gerenciador",
    }
}

pub fn version_from_output(id: &str, output: &str) -> String {
    let lines: Vec<&str> = output.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    if lines.is_empty() {
        return String::new();
    }
    match id {
        "winget" => {
            for line in &lines {
                if let Some(idx) = line.rfind('v') {
                    let rest = &line[idx..];
                    if rest.len() > 1 && rest.as_bytes()[1].is_ascii_digit() {
                        return rest.split_whitespace().next().unwrap_or(rest).trim_matches(|c: char| c == ',' || c == '.' && false).to_string();
                    }
                }
                if line.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    return (*line).to_string();
                }
            }
            lines[0].to_string()
        }
        "scoop" => lines
            .iter()
            .copied()
            .find(|line| !line.to_ascii_lowercase().starts_with("current scoop") && !line.ends_with(':'))
            .unwrap_or(lines[0])
            .to_string(),
        _ => lines[0].to_string(),
    }
}

pub fn watch_is_newer(latest: &str, seen: &str) -> bool {
    !latest.trim().is_empty() && !seen.trim().is_empty() && newer(latest, seen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str, source: &str, source_id: &str, winget: &str, repo: &str) -> Incoming {
        Incoming {
            name: name.into(),
            description: format!("{name} desc"),
            category: "imagem".into(),
            license: "GPL-3.0".into(),
            homepage: if repo.is_empty() { String::new() } else { format!("https://github.com/{repo}") },
            repo: repo.into(),
            winget_id: winget.into(),
            stars: 10,
            updated_ms: 10,
            open_source: source != "chocolatey",
            alternative_to: if name == "gimp" { "alternative ao photoshop".into() } else { String::new() },
            icon: String::new(),
            source: source.into(),
            source_id: source_id.into(),
            version: "1.0.0".into(),
            size: 20,
            portable: source == "scoop",
            installer: source != "scoop",
        }
    }

    #[test]
    fn merges_same_app_across_sources() {
        let hits = merge(vec![
            sample("gimp", "winget", "GIMP.GIMP", "GIMP.GIMP", ""),
            sample("GIMP", "scoop", "gimp", "", "GNOME/gimp"),
            sample("gimp", "curadoria", "gimp", "GIMP.GIMP", "GNOME/gimp"),
            sample("krita", "winget", "KDE.Krita", "KDE.Krita", "KDE/krita"),
        ]);
        assert_eq!(hits.len(), 2);
        let gimp = hits.iter().find(|hit| hit.name == "gimp").unwrap();
        assert!(gimp.sources.iter().any(|source| source == "winget"));
        assert!(gimp.sources.iter().any(|source| source == "scoop"));
        assert!(gimp.sources.iter().any(|source| source == "github"));
        assert!(gimp.open_source);
        assert!(gimp.portable && gimp.installer);
        assert_eq!(paid_label(&gimp.alternative_to), "photoshop");
        assert!(hits.iter().any(|hit| hit.name == "krita"));
    }

    #[test]
    fn does_not_merge_different_winget_ids() {
        let hits = merge(vec![
            sample("git", "winget", "Git.Git", "Git.Git", "git/git"),
            sample("git", "winget", "GitHub.GitHubDesktop", "GitHub.GitHubDesktop", "desktop/desktop"),
        ]);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn parses_scoop_and_choco_and_releases() {
        let scoop = parse_scoop(
            "vlc",
            r#"{"version":"3.0.21","description":"player","homepage":"https://github.com/videolan/vlc","license":"GPL-2.0","architecture":{"64bit":{"url":"https://example.com/vlc.zip"}}}"#,
        )
        .unwrap();
        assert_eq!(scoop.repo, "videolan/vlc");
        assert!(scoop.portable);
        let choco = parse_choco_atom(
            r#"<feed><entry><d:Id>git</d:Id><d:Title>Git</d:Title><d:Version>2.47.1</d:Version><d:Description>git &amp; friends</d:Description><d:Tags>git foss</d:Tags><d:ProjectUrl>https://github.com/git-for-windows/git</d:ProjectUrl><d:LicenseUrl>https://example.com/gpl</d:LicenseUrl><d:PackageSize m:type="Edm.Int64">42</d:PackageSize><d:Published>2024-11-01T00:00:00</d:Published><d:IsLatestVersion m:type="Edm.Boolean">true</d:IsLatestVersion></entry><entry><d:Id>old</d:Id><d:IsLatestVersion>false</d:IsLatestVersion></entry></feed>"#,
        );
        assert_eq!(choco.len(), 1);
        assert_eq!(choco[0].source_id, "git");
        assert_eq!(choco[0].description, "git & friends");
        assert!(choco[0].open_source);
        assert!(choco[0].updated_ms > 0);
        let releases = parse_github_releases(
            r#"[{"tag_name":"v2","body":"oi","html_url":"https://github.com/o/r/releases/tag/v2","published_at":"2024-05-01T00:00:00Z","assets":[{"name":"app-win64.exe","size":3,"browser_download_url":"https://example.com/a.exe"},{"name":"app.dmg","size":4,"browser_download_url":"https://example.com/a.dmg"}]}]"#,
        );
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].assets.len(), 1);
        assert_eq!(releases[0].assets[0].kind, "exe");
    }

    #[test]
    fn readme_strips_scripts_and_resolves_images() {
        let (html, images) = render_readme(
            "# ola\n\n<script>alert(1)</script>\n\n[site](javascript:alert(1))\n\n![tela](docs/tela.png)\n",
            "https://raw.githubusercontent.com/GNOME/gimp/HEAD/",
        );
        assert!(!html.to_lowercase().contains("script"));
        assert!(!html.to_lowercase().contains("javascript:"));
        assert!(html.contains("ola"));
        assert_eq!(images, vec!["https://raw.githubusercontent.com/GNOME/gimp/HEAD/docs/tela.png".to_string()]);
        assert!(html.contains("https://raw.githubusercontent.com/GNOME/gimp/HEAD/docs/tela.png"));
    }

    #[test]
    fn device_flow_and_manager_versions() {
        let start = parse_device_start(r#"{"device_code":"d","user_code":"ABCD-1234","verification_uri":"https://github.com/login/device","interval":5}"#).unwrap();
        assert_eq!(start.user_code, "ABCD-1234");
        assert!(matches!(parse_device_poll(r#"{"error":"authorization_pending"}"#), DevicePoll::Pending));
        assert!(matches!(parse_device_poll(r#"{"access_token":"sekret"}"#), DevicePoll::Granted { .. }));
        let winget = manager_from_probe("winget", r"C:\winget.exe", "v1.11.510\n");
        assert!(winget.installed);
        assert_eq!(winget.version, "v1.11.510");
        let missing = manager_from_probe("scoop", "", "");
        assert!(!missing.installed);
        assert_eq!(version_from_output("scoop", "Current Scoop version:\nb5c2e1f3 master\n"), "b5c2e1f3 master");
        assert_eq!(version_from_output("chocolatey", "2.4.1\n"), "2.4.1");
        assert!(scoop_install_script().contains("https://get.scoop.sh"));
        assert!(!scoop_install_script().to_lowercase().contains("runas"));
        assert!(chocolatey_install_script().contains("https://community.chocolatey.org/install.ps1"));
        assert!(watch_is_newer("2.0.0", "1.9.0"));
        assert!(!watch_is_newer("1.0.0", "1.0.0"));
        let assets = winget_installer_assets(
            r#"{"assets":[{"name":"Microsoft.DesktopAppInstaller.msixbundle","browser_download_url":"https://example.com/a.msixbundle","size":10},{"name":"DesktopAppInstaller_Dependencies.zip","browser_download_url":"https://example.com/d.zip","size":2},{"name":"notes.txt","browser_download_url":"https://example.com/n.txt","size":1}]}"#,
        );
        assert_eq!(assets.len(), 2);
        assert_eq!(rfc3339_ms("1970-01-01T00:00:00Z"), 0);
    }

    #[test]
    fn fts_query_quotes_tokens() {
        assert_eq!(fts_query("gimp photoshop"), "\"gimp\"* \"photoshop\"*");
        assert_eq!(fts_query("  "), "");
    }
}
