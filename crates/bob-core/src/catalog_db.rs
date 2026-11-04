//! Índice local do catálogo. A busca fica no sqlite, inclusive o fts.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::catalog_model::{
    github_repo, preset_collections, Asset, CatalogDetail, CatalogHit, Incoming, Offer, Release, fts_query, merge, norm_id,
    norm_name, norm_repo,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchQuery {
    pub text: String,
    pub category: String,
    pub open_source: Option<bool>,
    pub min_stars: u64,
    pub recent_days: u32,
    pub installed: Option<bool>,
    pub portable: Option<bool>,
    pub sort: String,
    pub offset: u32,
    pub limit: u32,
    pub section: String,
    pub collection_id: String,
    pub now_ms: i64,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            category: String::new(),
            open_source: None,
            min_stars: 0,
            recent_days: 0,
            installed: None,
            portable: None,
            sort: "relevancia".into(),
            offset: 0,
            limit: 80,
            section: String::new(),
            collection_id: String::new(),
            now_ms: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub total: u32,
    pub hits: Vec<CatalogHit>,
    pub categories: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub preset: bool,
    pub apps: Vec<String>,
}

pub struct CatalogIndex {
    conn: Connection,
}

impl CatalogIndex {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let conn = Connection::open(path).map_err(|err| err.to_string())?;
        let index = Self { conn };
        index.prepare()?;
        Ok(index)
    }

    pub fn memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|err| err.to_string())?;
        let index = Self { conn };
        index.prepare()?;
        Ok(index)
    }

    fn prepare(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS incoming (
                    source TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    payload TEXT NOT NULL,
                    PRIMARY KEY (source, source_id)
                );
                CREATE TABLE IF NOT EXISTS apps (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    norm_name TEXT NOT NULL,
                    description TEXT NOT NULL,
                    category TEXT NOT NULL,
                    license TEXT NOT NULL,
                    homepage TEXT NOT NULL,
                    repo TEXT NOT NULL,
                    winget_id TEXT NOT NULL,
                    stars INTEGER NOT NULL,
                    updated_ms INTEGER NOT NULL,
                    latest_version TEXT NOT NULL,
                    size INTEGER NOT NULL,
                    icon TEXT NOT NULL,
                    open_source INTEGER NOT NULL,
                    portable INTEGER NOT NULL,
                    installer INTEGER NOT NULL,
                    alternative_to TEXT NOT NULL,
                    installed INTEGER NOT NULL DEFAULT 0,
                    sources TEXT NOT NULL,
                    offers_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_apps_category ON apps(category);
                CREATE INDEX IF NOT EXISTS idx_apps_stars ON apps(stars);
                CREATE INDEX IF NOT EXISTS idx_apps_updated ON apps(updated_ms);
                CREATE INDEX IF NOT EXISTS idx_apps_norm ON apps(norm_name);
                CREATE VIRTUAL TABLE IF NOT EXISTS apps_fts USING fts5(
                    id UNINDEXED,
                    name,
                    description,
                    category,
                    winget_id,
                    alternative_to,
                    repo,
                    tokenize = 'unicode61 remove_diacritics 2'
                );
                CREATE TABLE IF NOT EXISTS favorites (app_id TEXT PRIMARY KEY);
                CREATE TABLE IF NOT EXISTS watches (app_id TEXT PRIMARY KEY, seen_version TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS collections (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    preset INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS collection_apps (
                    collection_id TEXT NOT NULL,
                    app_id TEXT NOT NULL,
                    PRIMARY KEY (collection_id, app_id)
                );
                CREATE TABLE IF NOT EXISTS details (
                    app_id TEXT PRIMARY KEY,
                    readme_html TEXT NOT NULL,
                    images_json TEXT NOT NULL,
                    releases_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS http_cache (
                    url TEXT PRIMARY KEY,
                    etag TEXT NOT NULL,
                    body TEXT NOT NULL,
                    fetched_ms INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                ",
            )
            .map_err(|err| err.to_string())?;
        let _ = self.conn.execute_batch("PRAGMA journal_mode = WAL;");
        Ok(())
    }

    pub fn app_count(&self) -> Result<u32, String> {
        self.conn
            .query_row("SELECT COUNT(*) FROM apps", [], |row| row.get(0))
            .map_err(|err| err.to_string())
    }

    pub fn replace_source(&self, source: &str, rows: &[Incoming]) -> Result<(), String> {
        let tx = self.conn.unchecked_transaction().map_err(|err| err.to_string())?;
        tx.execute("DELETE FROM incoming WHERE source = ?1", [source]).map_err(|err| err.to_string())?;
        for row in rows {
            let payload = serde_json::to_string(row).map_err(|err| err.to_string())?;
            let source_id = if row.source_id.is_empty() { row.name.clone() } else { row.source_id.clone() };
            tx.execute(
                "INSERT INTO incoming (source, source_id, payload) VALUES (?1, ?2, ?3)",
                params![source, source_id, payload],
            )
            .map_err(|err| err.to_string())?;
        }
        tx.commit().map_err(|err| err.to_string())
    }

    pub fn load_incoming(&self) -> Result<Vec<Incoming>, String> {
        let mut stmt = self.conn.prepare("SELECT payload FROM incoming").map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| err.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            let payload = row.map_err(|err| err.to_string())?;
            if let Ok(item) = serde_json::from_str::<Incoming>(&payload) {
                out.push(item);
            }
        }
        Ok(out)
    }

    pub fn write_merged(&self, hits: &[CatalogHit]) -> Result<(), String> {
        let installed = self.installed_keys()?;
        let tx = self.conn.unchecked_transaction().map_err(|err| err.to_string())?;
        tx.execute_batch("DELETE FROM apps; DELETE FROM apps_fts;").map_err(|err| err.to_string())?;
        for hit in hits {
            let mut hit = hit.clone();
            let winget = norm_id(&hit.winget_id);
            let name = norm_name(&hit.name);
            hit.installed = installed.iter().any(|(id, nm)| (!id.is_empty() && id == &winget) || (!nm.is_empty() && nm == &name));
            let offers = serde_json::to_string(&hit.offers).unwrap_or_else(|_| "[]".into());
            let sources = hit.sources.join(",");
            tx.execute(
                "INSERT INTO apps (
                    id, name, norm_name, description, category, license, homepage, repo, winget_id,
                    stars, updated_ms, latest_version, size, icon, open_source, portable, installer,
                    alternative_to, installed, sources, offers_json
                ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
                params![
                    hit.id,
                    hit.name,
                    name,
                    hit.description,
                    hit.category,
                    hit.license,
                    hit.homepage,
                    hit.repo,
                    hit.winget_id,
                    hit.stars as i64,
                    hit.updated_ms,
                    hit.latest_version,
                    hit.size as i64,
                    hit.icon,
                    hit.open_source as i64,
                    hit.portable as i64,
                    hit.installer as i64,
                    hit.alternative_to,
                    hit.installed as i64,
                    sources,
                    offers,
                ],
            )
            .map_err(|err| err.to_string())?;
            tx.execute(
                "INSERT INTO apps_fts (id, name, description, category, winget_id, alternative_to, repo) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![hit.id, hit.name, hit.description, hit.category, hit.winget_id, hit.alternative_to, hit.repo],
            )
            .map_err(|err| err.to_string())?;
        }
        for (id, name, members) in preset_collections() {
            let exists: i64 = tx
                .query_row("SELECT COUNT(*) FROM collections WHERE id = ?1", [id], |row| row.get(0))
                .unwrap_or(0);
            if exists > 0 {
                continue;
            }
            tx.execute("INSERT INTO collections (id, name, preset) VALUES (?1, ?2, 1)", params![id, name])
                .map_err(|err| err.to_string())?;
            for member in members {
                let wanted = norm_name(member);
                if let Ok(app_id) = tx.query_row("SELECT id FROM apps WHERE norm_name = ?1 LIMIT 1", [&wanted], |row| row.get::<_, String>(0)) {
                    tx.execute(
                        "INSERT OR IGNORE INTO collection_apps (collection_id, app_id) VALUES (?1, ?2)",
                        params![id, app_id],
                    )
                    .map_err(|err| err.to_string())?;
                }
            }
        }
        tx.commit().map_err(|err| err.to_string())
    }

    pub fn rebuild(&self) -> Result<usize, String> {
        let incoming = self.load_incoming()?;
        let hits = merge(incoming);
        let count = hits.len();
        self.write_merged(&hits)?;
        Ok(count)
    }

    pub fn ensure_curated(&self) -> Result<(), String> {
        if self.app_count()? > 0 {
            return Ok(());
        }
        self.replace_source("curadoria", &crate::catalog_model::curated_incoming())?;
        self.rebuild()?;
        Ok(())
    }

    fn installed_keys(&self) -> Result<Vec<(String, String)>, String> {
        let raw: String = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = 'installed'", [], |row| row.get(0))
            .optional()
            .map_err(|err| err.to_string())?
            .unwrap_or_default();
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn set_installed(&self, winget_ids: &[String], names: &[String]) -> Result<(), String> {
        let keys: Vec<(String, String)> = winget_ids
            .iter()
            .map(|id| (norm_id(id), String::new()))
            .chain(names.iter().map(|name| (String::new(), norm_name(name))))
            .collect();
        let payload = serde_json::to_string(&keys).unwrap_or_else(|_| "[]".into());
        self.conn
            .execute("INSERT INTO meta (key, value) VALUES ('installed', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value", [payload])
            .map_err(|err| err.to_string())?;
        self.conn.execute("UPDATE apps SET installed = 0", []).map_err(|err| err.to_string())?;
        for id in winget_ids {
            self.conn
                .execute("UPDATE apps SET installed = 1 WHERE lower(winget_id) = ?1", [norm_id(id)])
                .map_err(|err| err.to_string())?;
        }
        for name in names {
            self.conn
                .execute("UPDATE apps SET installed = 1 WHERE norm_name = ?1", [norm_name(name)])
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    pub fn search(&self, query: &SearchQuery) -> Result<SearchPage, String> {
        let mut query = query.clone();
        if query.limit == 0 {
            query.limit = 80;
        }
        query.limit = query.limit.min(400);
        if query.section == "trending" && query.recent_days == 0 {
            query.recent_days = 180;
            if query.sort == "relevancia" {
                query.sort = "estrelas".into();
            }
        }
        if query.section == "stars" && query.sort == "relevancia" {
            query.sort = "estrelas".into();
        }
        let fts = fts_query(&query.text);
        let use_fts = !fts.is_empty();
        let mut where_sql = String::from(" WHERE 1=1 ");
        let mut args: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if use_fts {
            where_sql.push_str(" AND apps_fts MATCH ? ");
            args.push(Box::new(fts));
        }
        if !query.category.is_empty() {
            where_sql.push_str(" AND a.category = ? ");
            args.push(Box::new(query.category.clone()));
        }
        if let Some(open) = query.open_source {
            where_sql.push_str(" AND a.open_source = ? ");
            args.push(Box::new(open as i64));
        }
        if query.min_stars > 0 {
            where_sql.push_str(" AND a.stars >= ? ");
            args.push(Box::new(query.min_stars as i64));
        }
        if query.recent_days > 0 && query.now_ms > 0 {
            let cutoff = query.now_ms - i64::from(query.recent_days) * 86_400_000;
            where_sql.push_str(" AND a.updated_ms >= ? ");
            args.push(Box::new(cutoff));
        }
        if let Some(installed) = query.installed {
            where_sql.push_str(" AND a.installed = ? ");
            args.push(Box::new(installed as i64));
        }
        if let Some(portable) = query.portable {
            if portable {
                where_sql.push_str(" AND a.portable = 1 ");
            } else {
                where_sql.push_str(" AND a.installer = 1 ");
            }
        }
        if query.section == "alternatives" {
            where_sql.push_str(" AND a.alternative_to != '' ");
        }
        let join_fav = query.section == "favorites";
        let join_col = query.section == "collection" && !query.collection_id.is_empty();
        if join_col {
            where_sql.push_str(" AND c.collection_id = ? ");
            args.push(Box::new(query.collection_id.clone()));
        }
        let order = match query.sort.as_str() {
            "estrelas" => "a.stars DESC, a.name COLLATE NOCASE",
            "atualizacao" => "a.updated_ms DESC, a.name COLLATE NOCASE",
            "nome" => "a.name COLLATE NOCASE",
            _ if use_fts => "bm25(apps_fts)",
            _ => "a.stars DESC, a.name COLLATE NOCASE",
        };
        let from = if use_fts {
            "FROM apps a JOIN apps_fts ON apps_fts.id = a.id"
        } else {
            "FROM apps a"
        };
        let extra = if join_fav {
            " JOIN favorites f ON f.app_id = a.id "
        } else if join_col {
            " JOIN collection_apps c ON c.app_id = a.id "
        } else {
            ""
        };
        let count_sql = format!("SELECT COUNT(*) {from} {extra} {where_sql}");
        let mut count_stmt = self.conn.prepare(&count_sql).map_err(|err| err.to_string())?;
        let arg_refs: Vec<&dyn rusqlite::types::ToSql> = args.iter().map(|arg| arg.as_ref()).collect();
        let total: i64 = count_stmt.query_row(arg_refs.as_slice(), |row| row.get(0)).map_err(|err| err.to_string())?;
        let page_sql = format!(
            "SELECT a.id, a.name, a.description, a.category, a.license, a.homepage, a.repo, a.winget_id,
                    a.stars, a.updated_ms, a.latest_version, a.size, a.icon, a.open_source, a.portable, a.installer,
                    a.alternative_to, a.sources, a.offers_json, a.installed,
                    EXISTS(SELECT 1 FROM favorites fav WHERE fav.app_id = a.id),
                    EXISTS(SELECT 1 FROM watches w WHERE w.app_id = a.id)
             {from} {extra} {where_sql}
             ORDER BY {order}
             LIMIT ? OFFSET ?"
        );
        let mut page_args = args;
        page_args.push(Box::new(query.limit as i64));
        page_args.push(Box::new(query.offset as i64));
        let mut stmt = self.conn.prepare(&page_sql).map_err(|err| err.to_string())?;
        let arg_refs: Vec<&dyn rusqlite::types::ToSql> = page_args.iter().map(|arg| arg.as_ref()).collect();
        let rows = stmt.query_map(arg_refs.as_slice(), hit_from_row).map_err(|err| err.to_string())?;
        let mut hits = Vec::new();
        for row in rows {
            hits.push(row.map_err(|err| err.to_string())?);
        }
        let mut categories = Vec::new();
        let mut cat_stmt = self
            .conn
            .prepare("SELECT DISTINCT category FROM apps WHERE category != '' ORDER BY category COLLATE NOCASE")
            .map_err(|err| err.to_string())?;
        let cats = cat_stmt.query_map([], |row| row.get::<_, String>(0)).map_err(|err| err.to_string())?;
        for cat in cats {
            categories.push(cat.map_err(|err| err.to_string())?);
        }
        Ok(SearchPage {
            total: u32::try_from(total).unwrap_or(u32::MAX),
            hits,
            categories,
        })
    }

    pub fn hit(&self, id: &str) -> Result<Option<CatalogHit>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT a.id, a.name, a.description, a.category, a.license, a.homepage, a.repo, a.winget_id,
                        a.stars, a.updated_ms, a.latest_version, a.size, a.icon, a.open_source, a.portable, a.installer,
                        a.alternative_to, a.sources, a.offers_json, a.installed,
                        EXISTS(SELECT 1 FROM favorites fav WHERE fav.app_id = a.id),
                        EXISTS(SELECT 1 FROM watches w WHERE w.app_id = a.id)
                 FROM apps a WHERE a.id = ?1",
            )
            .map_err(|err| err.to_string())?;
        stmt.query_row([id], hit_from_row).optional().map_err(|err| err.to_string())
    }

    pub fn detail(&self, id: &str) -> Result<Option<CatalogDetail>, String> {
        let Some(hit) = self.hit(id)? else {
            return Ok(None);
        };
        let cached = self
            .conn
            .query_row(
                "SELECT readme_html, images_json, releases_json FROM details WHERE app_id = ?1",
                [id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            )
            .optional()
            .map_err(|err| err.to_string())?;
        let (readme_html, images, releases) = if let Some((html, images, releases)) = cached {
            (
                html,
                serde_json::from_str::<Vec<String>>(&images).unwrap_or_default(),
                serde_json::from_str::<Vec<Release>>(&releases).unwrap_or_default(),
            )
        } else {
            (String::new(), Vec::new(), Vec::new())
        };
        let alternatives = self.alternatives_for(&hit)?;
        Ok(Some(CatalogDetail {
            hit,
            readme_html,
            images,
            releases,
            alternatives,
        }))
    }

    fn alternatives_for(&self, hit: &CatalogHit) -> Result<Vec<CatalogHit>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT a.id, a.name, a.description, a.category, a.license, a.homepage, a.repo, a.winget_id,
                        a.stars, a.updated_ms, a.latest_version, a.size, a.icon, a.open_source, a.portable, a.installer,
                        a.alternative_to, a.sources, a.offers_json, a.installed,
                        EXISTS(SELECT 1 FROM favorites fav WHERE fav.app_id = a.id),
                        EXISTS(SELECT 1 FROM watches w WHERE w.app_id = a.id)
                 FROM apps a
                 WHERE a.id != ?1 AND (a.category = ?2 OR (a.alternative_to != '' AND a.alternative_to = ?3))
                 ORDER BY a.stars DESC, a.name COLLATE NOCASE
                 LIMIT 6",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![hit.id, hit.category, hit.alternative_to], hit_from_row)
            .map_err(|err| err.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|err| err.to_string())?);
        }
        Ok(out)
    }

    pub fn save_detail(&self, id: &str, html: &str, images: &[String], releases: &[Release]) -> Result<(), String> {
        let images = serde_json::to_string(images).unwrap_or_else(|_| "[]".into());
        let releases = serde_json::to_string(releases).unwrap_or_else(|_| "[]".into());
        self.conn
            .execute(
                "INSERT INTO details (app_id, readme_html, images_json, releases_json) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(app_id) DO UPDATE SET readme_html = excluded.readme_html, images_json = excluded.images_json, releases_json = excluded.releases_json",
                params![id, html, images, releases],
            )
            .map_err(|err| err.to_string())?;
        if let Ok(list) = serde_json::from_str::<Vec<Release>>(&releases) {
            if let Some(first) = list.iter().find(|release| !release.version.is_empty()) {
                let current: String = self
                    .conn
                    .query_row("SELECT latest_version FROM apps WHERE id = ?1", [id], |row| row.get(0))
                    .unwrap_or_default();
                if current.is_empty() || crate::version::newer(&first.version, &current) {
                    self.conn
                        .execute(
                            "UPDATE apps SET latest_version = ?1, updated_ms = CASE WHEN ?2 > updated_ms THEN ?2 ELSE updated_ms END WHERE id = ?3",
                            params![first.version, first.published_ms, id],
                        )
                        .map_err(|err| err.to_string())?;
                }
                if let Some(asset) = first.assets.iter().max_by_key(|asset| asset.size) {
                    self.attach_github_offer(id, &first.version, asset)?;
                }
            }
        }
        Ok(())
    }

    fn attach_github_offer(&self, id: &str, version: &str, asset: &Asset) -> Result<(), String> {
        let Some(mut hit) = self.hit(id)? else {
            return Ok(());
        };
        let portable = asset.kind == "zip";
        let installer = asset.kind == "exe" || asset.kind == "msi";
        if let Some(offer) = hit.offers.iter_mut().find(|offer| offer.source == "github") {
            offer.version = version.into();
            offer.size = asset.size;
            offer.portable = portable;
            offer.installer = installer;
        } else if hit.repo.contains('/') {
            hit.offers.push(Offer {
                source: "github".into(),
                source_id: hit.repo.clone(),
                version: version.into(),
                size: asset.size,
                portable,
                installer,
            });
        }
        hit.portable = hit.offers.iter().any(|offer| offer.portable);
        hit.installer = hit.offers.iter().any(|offer| offer.installer);
        if !hit.sources.iter().any(|source| source == "github") && hit.repo.contains('/') {
            hit.sources.push("github".into());
        }
        let offers = serde_json::to_string(&hit.offers).unwrap_or_else(|_| "[]".into());
        let sources = hit.sources.join(",");
        self.conn
            .execute(
                "UPDATE apps SET offers_json = ?1, sources = ?2, portable = ?3, installer = ?4, size = CASE WHEN ?5 > size THEN ?5 ELSE size END WHERE id = ?6",
                params![offers, sources, hit.portable as i64, hit.installer as i64, asset.size as i64, id],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn update_repo(&self, repo: &str, stars: u64, description: &str, updated_ms: i64, license: &str) -> Result<(), String> {
        let repo = norm_repo(repo);
        if !repo.contains('/') {
            return Ok(());
        }
        self.conn
            .execute(
                "UPDATE apps SET
                    stars = CASE WHEN ?1 > stars THEN ?1 ELSE stars END,
                    description = CASE WHEN description = '' THEN ?2 ELSE description END,
                    updated_ms = CASE WHEN ?3 > updated_ms THEN ?3 ELSE updated_ms END,
                    license = CASE WHEN license = '' THEN ?4 ELSE license END
                 WHERE repo = ?5",
                params![stars as i64, description, updated_ms, license, repo],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn repos_missing_stars(&self, limit: u32) -> Result<Vec<(String, String)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, repo FROM apps WHERE repo LIKE '%/%' AND stars = 0 ORDER BY name COLLATE NOCASE LIMIT ?1")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([limit as i64], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|err| err.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|err| err.to_string())?);
        }
        Ok(out)
    }

    pub fn set_favorite(&self, id: &str, on: bool) -> Result<(), String> {
        if on {
            self.conn.execute("INSERT OR IGNORE INTO favorites (app_id) VALUES (?1)", [id])
        } else {
            self.conn.execute("DELETE FROM favorites WHERE app_id = ?1", [id])
        }
        .map(|_| ())
        .map_err(|err| err.to_string())
    }

    pub fn set_watch(&self, id: &str, seen_version: &str, on: bool) -> Result<(), String> {
        if on {
            self.conn.execute(
                "INSERT INTO watches (app_id, seen_version) VALUES (?1, ?2)
                 ON CONFLICT(app_id) DO UPDATE SET seen_version = excluded.seen_version",
                params![id, seen_version],
            )
        } else {
            self.conn.execute("DELETE FROM watches WHERE app_id = ?1", [id])
        }
        .map(|_| ())
        .map_err(|err| err.to_string())
    }

    pub fn watches(&self) -> Result<Vec<(String, String, String, String)>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT w.app_id, w.seen_version, a.latest_version, a.name
                 FROM watches w JOIN apps a ON a.id = w.app_id",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
            .map_err(|err| err.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|err| err.to_string())?);
        }
        Ok(out)
    }

    pub fn collections(&self) -> Result<Vec<Collection>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, preset FROM collections ORDER BY preset DESC, name COLLATE NOCASE")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)))
            .map_err(|err| err.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, preset) = row.map_err(|err| err.to_string())?;
            let mut apps_stmt = self
                .conn
                .prepare("SELECT app_id FROM collection_apps WHERE collection_id = ?1 ORDER BY app_id")
                .map_err(|err| err.to_string())?;
            let apps = apps_stmt
                .query_map([&id], |row| row.get::<_, String>(0))
                .map_err(|err| err.to_string())?;
            let mut list = Vec::new();
            for app in apps {
                list.push(app.map_err(|err| err.to_string())?);
            }
            out.push(Collection {
                id,
                name,
                preset: preset != 0,
                apps: list,
            });
        }
        Ok(out)
    }

    pub fn create_collection(&self, name: &str) -> Result<Collection, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("dê um nome à coleção".into());
        }
        let id = format!("col:{}", norm_name(name));
        self.conn
            .execute("INSERT OR IGNORE INTO collections (id, name, preset) VALUES (?1, ?2, 0)", params![id, name])
            .map_err(|err| err.to_string())?;
        Ok(Collection {
            id,
            name: name.into(),
            preset: false,
            apps: Vec::new(),
        })
    }

    pub fn set_collection_app(&self, collection_id: &str, app_id: &str, on: bool) -> Result<(), String> {
        if on {
            self.conn.execute(
                "INSERT OR IGNORE INTO collection_apps (collection_id, app_id) VALUES (?1, ?2)",
                params![collection_id, app_id],
            )
        } else {
            self.conn.execute(
                "DELETE FROM collection_apps WHERE collection_id = ?1 AND app_id = ?2",
                params![collection_id, app_id],
            )
        }
        .map(|_| ())
        .map_err(|err| err.to_string())
    }

    pub fn http_lookup(&self, url: &str) -> Result<Option<(String, String)>, String> {
        self.conn
            .query_row("SELECT etag, body FROM http_cache WHERE url = ?1", [url], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional()
            .map_err(|err| err.to_string())
    }

    pub fn http_store(&self, url: &str, etag: &str, body: &str, fetched_ms: i64) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO http_cache (url, etag, body, fetched_ms) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(url) DO UPDATE SET etag = excluded.etag, body = excluded.body, fetched_ms = excluded.fetched_ms",
                params![url, etag, body, fetched_ms],
            )
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    pub fn meta_get(&self, key: &str) -> Result<String, String> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| row.get(0))
            .optional()
            .map(|value| value.unwrap_or_default())
            .map_err(|err| err.to_string())
    }

    pub fn meta_set(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map(|_| ())
            .map_err(|err| err.to_string())
    }
}

fn hit_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CatalogHit> {
    let sources: String = row.get(17)?;
    let offers: String = row.get(18)?;
    Ok(CatalogHit {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        category: row.get(3)?,
        license: row.get(4)?,
        homepage: row.get(5)?,
        repo: row.get(6)?,
        winget_id: row.get(7)?,
        stars: row.get::<_, i64>(8)? as u64,
        updated_ms: row.get(9)?,
        latest_version: row.get(10)?,
        size: row.get::<_, i64>(11)? as u64,
        icon: row.get(12)?,
        open_source: row.get::<_, i64>(13)? != 0,
        portable: row.get::<_, i64>(14)? != 0,
        installer: row.get::<_, i64>(15)? != 0,
        alternative_to: row.get(16)?,
        sources: sources.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
        offers: serde_json::from_str(&offers).unwrap_or_default(),
        installed: row.get::<_, i64>(19)? != 0,
        favorite: row.get::<_, i64>(20)? != 0,
        watched: row.get::<_, i64>(21)? != 0,
    })
}

pub fn read_winget_index(path: &Path) -> Result<Vec<Incoming>, String> {
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|err| err.to_string())?;
    if table_exists(&conn, "packages")? {
        return read_packages(&conn);
    }
    if table_exists(&conn, "manifest")? {
        return read_legacy(&conn);
    }
    Err("índice do winget sem tabela de pacotes".into())
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .map_err(|err| err.to_string())
}

fn columns(conn: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})")).map_err(|err| err.to_string())?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1)).map_err(|err| err.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|err| err.to_string())?);
    }
    Ok(out)
}

fn read_packages(conn: &Connection) -> Result<Vec<Incoming>, String> {
    let cols = columns(conn, "packages")?;
    let has = |name: &str| cols.iter().any(|col| col.eq_ignore_ascii_case(name));
    if !has("id") || !has("name") {
        return Err("tabela packages sem id".into());
    }
    let version_col = if has("latest_version") {
        "latest_version"
    } else if has("version") {
        "version"
    } else {
        ""
    };
    let moniker = if has("moniker") { "moniker" } else { "NULL" };
    let version_expr = if version_col.is_empty() { "''".to_string() } else { version_col.to_string() };
    let sql = format!("SELECT typeof(id), id, name, {moniker}, {version_expr} FROM packages");
    let mut stmt = conn.prepare(&sql).map_err(|err| err.to_string())?;
    let mut probe = stmt.query([]).map_err(|err| err.to_string())?;
    let Some(first) = probe.next().map_err(|err| err.to_string())? else {
        return Ok(Vec::new());
    };
    let kind: String = first.get(0).unwrap_or_default();
    drop(probe);
    drop(stmt);
    if kind == "text" {
        let sql = format!("SELECT id, name, ifnull({moniker}, ''), ifnull({version_expr}, '') FROM packages");
        collect_winget(conn, &sql, true)
    } else {
        let version_join = if version_col.is_empty() {
            "''".into()
        } else if table_exists(conn, "versions")? {
            format!("ifnull((SELECT version FROM versions WHERE rowid = packages.{version_col}), '')")
        } else {
            format!("ifnull(packages.{version_col}, '')")
        };
        let name_expr = if table_exists(conn, "names")? {
            "ifnull((SELECT name FROM names WHERE rowid = packages.name), '')"
        } else {
            "packages.name"
        };
        let id_expr = if table_exists(conn, "ids")? {
            "ifnull((SELECT id FROM ids WHERE rowid = packages.id), '')"
        } else {
            "packages.id"
        };
        let moniker_expr = if has("moniker") && table_exists(conn, "monikers")? {
            "ifnull((SELECT moniker FROM monikers WHERE rowid = packages.moniker), '')"
        } else {
            "''"
        };
        let sql = format!("SELECT {id_expr}, {name_expr}, {moniker_expr}, {version_join} FROM packages");
        collect_winget(conn, &sql, false)
    }
}

fn read_legacy(conn: &Connection) -> Result<Vec<Incoming>, String> {
    let sql = "
        SELECT i.id, n.name, ifnull(m.moniker, ''), v.version
        FROM manifest man
        JOIN ids i ON man.id = i.rowid
        JOIN names n ON man.name = n.rowid
        LEFT JOIN monikers m ON man.moniker = m.rowid
        JOIN versions v ON man.version = v.rowid
    ";
    collect_winget(conn, sql, false)
}

fn collect_winget(conn: &Connection, sql: &str, _text: bool) -> Result<Vec<Incoming>, String> {
    let mut stmt = conn.prepare(sql).map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?)))
        .map_err(|err| err.to_string())?;
    let mut best: std::collections::HashMap<String, Incoming> = std::collections::HashMap::new();
    for row in rows {
        let (id, name, moniker, version) = row.map_err(|err| err.to_string())?;
        let id = id.trim().to_string();
        if id.is_empty() {
            continue;
        }
        let mut display = name.trim().to_string();
        if display.is_empty() || display.eq_ignore_ascii_case("none") {
            display = if moniker.is_empty() || moniker.eq_ignore_ascii_case("none") { id.clone() } else { moniker.clone() };
        }
        let portable = display.to_ascii_lowercase().contains("portable") || moniker.to_ascii_lowercase().contains("portable");
        let item = Incoming {
            name: display,
            description: String::new(),
            category: "outros".into(),
            license: String::new(),
            homepage: String::new(),
            repo: String::new(),
            winget_id: id.clone(),
            stars: 0,
            updated_ms: 0,
            open_source: false,
            alternative_to: String::new(),
            icon: String::new(),
            source: "winget".into(),
            source_id: id.clone(),
            version: version.trim().to_string(),
            size: 0,
            portable,
            installer: true,
        };
        match best.get(&id.to_ascii_lowercase()) {
            Some(prev) if !item.version.is_empty() && !crate::version::newer(&item.version, &prev.version) && item.version != prev.version => {}
            _ => {
                best.insert(id.to_ascii_lowercase(), item);
            }
        }
    }
    Ok(best.into_values().collect())
}

pub fn scoop_from_zip(bytes: &[u8]) -> Result<Vec<Incoming>, String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|err| err.to_string())?;
    let mut out = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|err| err.to_string())?;
        let name = file.name().to_string();
        if file.is_dir() || !name.to_ascii_lowercase().ends_with(".json") || !name.replace('\\', "/").contains("/bucket/") && !name.contains("bucket/") {
            continue;
        }
        let stem = Path::new(&name).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if stem.is_empty() || stem.starts_with('.') {
            continue;
        }
        let mut body = String::new();
        use std::io::Read;
        file.read_to_string(&mut body).map_err(|err| err.to_string())?;
        if let Some(item) = crate::catalog_model::parse_scoop(&stem, &body) {
            out.push(item);
        }
    }
    Ok(out)
}

pub fn extract_matching(archive_path: &Path, dest: &Path, exts: &[&str]) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(dest).map_err(|err| err.to_string())?;
    let file = std::fs::File::open(archive_path).map_err(|err| err.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|err| err.to_string())?;
    let mut out = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|err| err.to_string())?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().replace('\\', "/");
        let lower = name.to_ascii_lowercase();
        if !exts.iter().any(|ext| lower.ends_with(ext)) {
            continue;
        }
        let file_name = Path::new(&name).file_name().and_then(|s| s.to_str()).unwrap_or("arquivo");
        let target = dest.join(file_name);
        let mut output = std::fs::File::create(&target).map_err(|err| err.to_string())?;
        std::io::copy(&mut file, &mut output).map_err(|err| err.to_string())?;
        out.push(target);
    }
    Ok(out)
}

pub fn find_index_db(archive_path: &Path, dest: &Path) -> Result<PathBuf, String> {
    let found = extract_matching(archive_path, dest, &["index.db"])?;
    found.into_iter().next().ok_or_else(|| "o pacote do winget não trouxe index.db".into())
}

pub fn github_repo_of(value: &str) -> Option<String> {
    github_repo(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog_model::{curated_incoming, parse_scoop};

    fn gimp_sources() -> Vec<Incoming> {
        let mut rows = vec![parse_scoop(
            "gimp",
            r#"{"version":"2.10.38","description":"editor","homepage":"https://github.com/GNOME/gimp","license":"GPL-3.0"}"#,
        )
        .unwrap()];
        rows.push(Incoming {
            name: "GIMP".into(),
            description: String::new(),
            category: "outros".into(),
            license: String::new(),
            homepage: String::new(),
            repo: String::new(),
            winget_id: "GIMP.GIMP".into(),
            stars: 0,
            updated_ms: 0,
            open_source: false,
            alternative_to: String::new(),
            icon: String::new(),
            source: "winget".into(),
            source_id: "GIMP.GIMP".into(),
            version: "2.10.38".into(),
            size: 1000,
            portable: false,
            installer: true,
        });
        let mut curated = curated_incoming().into_iter().find(|item| norm_name(&item.name) == "gimp").unwrap();
        curated.stars = 5000;
        rows.push(curated);
        rows
    }

    #[test]
    fn search_filters_and_collections() {
        let index = CatalogIndex::memory().unwrap();
        let rows = gimp_sources();
        index.replace_source("scoop", &rows.iter().filter(|row| row.source == "scoop").cloned().collect::<Vec<_>>()).unwrap();
        index.replace_source("winget", &rows.iter().filter(|row| row.source == "winget").cloned().collect::<Vec<_>>()).unwrap();
        index.replace_source("curadoria", &rows.iter().filter(|row| row.source == "curadoria").cloned().collect::<Vec<_>>()).unwrap();
        let mut other = rows.iter().find(|row| row.source == "winget").unwrap().clone();
        other.name = "krita".into();
        other.winget_id = "KDE.Krita".into();
        other.source_id = "KDE.Krita".into();
        other.repo = "kde/krita".into();
        other.homepage = "https://github.com/KDE/krita".into();
        other.stars = 100;
        other.updated_ms = 1_700_000_000_000;
        other.open_source = true;
        other.alternative_to = "alternative ao photoshop".into();
        other.category = "imagem".into();
        let mut winget_rows = rows.iter().filter(|row| row.source == "winget").cloned().collect::<Vec<_>>();
        winget_rows.push(other);
        index.replace_source("winget", &winget_rows).unwrap();
        assert!(index.rebuild().unwrap() >= 2);
        index.set_installed(&["GIMP.GIMP".into()], &[]).unwrap();
        let mut query = SearchQuery {
            text: "photoshop".into(),
            now_ms: 1_800_000_000_000,
            ..SearchQuery::default()
        };
        let page = index.search(&query).unwrap();
        assert!(page.total >= 1);
        assert!(page.hits.iter().any(|hit| hit.name.to_lowercase().contains("gimp") || hit.alternative_to.contains("photoshop")));
        query.text.clear();
        query.min_stars = 1000;
        query.open_source = Some(true);
        query.installed = Some(true);
        let page = index.search(&query).unwrap();
        assert_eq!(page.hits.len(), 1);
        assert!(page.hits[0].installed);
        assert!(page.hits[0].sources.iter().any(|source| source == "winget"));
        let collections = index.collections().unwrap();
        assert!(collections.iter().any(|col| col.name == "mídia" && col.apps.iter().any(|id| id.contains("gimp") || id.starts_with("wg:"))));
        let gimp = page.hits[0].id.clone();
        index.set_favorite(&gimp, true).unwrap();
        index.set_watch(&gimp, "2.10.0", true).unwrap();
        query.installed = None;
        query.min_stars = 0;
        query.section = "favorites".into();
        assert_eq!(index.search(&query).unwrap().hits.len(), 1);
        let made = index.create_collection("meus").unwrap();
        index.set_collection_app(&made.id, &gimp, true).unwrap();
        query.section = "collection".into();
        query.collection_id = made.id;
        assert_eq!(index.search(&query).unwrap().total, 1);
        let detail = index.detail(&gimp).unwrap().unwrap();
        assert!(!detail.alternatives.is_empty() || detail.hit.category == "imagem");
    }

    #[test]
    fn reads_both_winget_schemas_and_scoop_zip() {
        let dir = std::env::temp_dir().join(format!("bob-catalog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let flat = dir.join("flat.db");
        let conn = Connection::open(&flat).unwrap();
        conn.execute_batch(
            "CREATE TABLE packages (id TEXT, name TEXT, moniker TEXT, latest_version TEXT);
             INSERT INTO packages VALUES ('VideoLAN.VLC', 'VLC', 'vlc', '3.0.21');",
        )
        .unwrap();
        drop(conn);
        let rows = read_winget_index(&flat).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].winget_id, "VideoLAN.VLC");
        assert_eq!(rows[0].version, "3.0.21");

        let legacy = dir.join("legacy.db");
        let conn = Connection::open(&legacy).unwrap();
        conn.execute_batch(
            "CREATE TABLE ids (rowid INTEGER PRIMARY KEY, id TEXT);
             CREATE TABLE names (rowid INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE monikers (rowid INTEGER PRIMARY KEY, moniker TEXT);
             CREATE TABLE versions (rowid INTEGER PRIMARY KEY, version TEXT);
             CREATE TABLE manifest (rowid INTEGER PRIMARY KEY, id INT, name INT, moniker INT, version INT);
             INSERT INTO ids (rowid, id) VALUES (1, '7zip.7zip');
             INSERT INTO names (rowid, name) VALUES (1, '7-Zip');
             INSERT INTO monikers (rowid, moniker) VALUES (1, '7zip');
             INSERT INTO versions (rowid, version) VALUES (1, '24.08');
             INSERT INTO manifest (id, name, moniker, version) VALUES (1, 1, 1, 1);",
        )
        .unwrap();
        drop(conn);
        let rows = read_winget_index(&legacy).unwrap();
        assert_eq!(rows[0].name, "7-Zip");
        assert_eq!(rows[0].source_id, "7zip.7zip");

        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            writer.start_file("main/bucket/bat.json", zip::write::SimpleFileOptions::default()).unwrap();
            use std::io::Write;
            writer
                .write_all(br#"{"version":"0.24.0","description":"cat com asas","homepage":"https://github.com/sharkdp/bat","license":"MIT"}"#)
                .unwrap();
            writer.finish().unwrap();
        }
        let parsed = scoop_from_zip(cursor.get_ref()).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].source_id, "bat");
        assert_eq!(parsed[0].repo, "sharkdp/bat");

        let msix = dir.join("source.msix");
        {
            let file = std::fs::File::create(&msix).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            writer.start_file("Public/index.db", zip::write::SimpleFileOptions::default()).unwrap();
            use std::io::Write;
            writer.write_all(b"sqlite-bytes").unwrap();
            writer.finish().unwrap();
        }
        let extracted = find_index_db(&msix, &dir.join("out")).unwrap();
        assert_eq!(std::fs::read(&extracted).unwrap(), b"sqlite-bytes");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
