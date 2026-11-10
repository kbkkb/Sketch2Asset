use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub name: String,
    pub style: String,
    pub style_prompt: String,
    #[serde(default)]
    pub reference_images: Vec<String>,
    pub asset_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetItem {
    pub id: String,
    pub project: String,
    #[serde(default)]
    pub archetype: String,
    pub file_path: String,
    pub prompt: String,
    pub model: String,
    pub via: String,
    pub width: u32,
    pub height: u32,
    pub created_at: i64,
}

pub struct LibraryDb {
    conn: Mutex<Connection>,
}

impl LibraryDb {
    pub fn open(db_dir: &Path) -> AppResult<Self> {
        fs::create_dir_all(db_dir)?;
        let db_path = db_dir.join("library.db");
        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS projects (
                 name TEXT PRIMARY KEY,
                 style TEXT NOT NULL DEFAULT '',
                 style_prompt TEXT NOT NULL DEFAULT '',
                 reference_images_json TEXT NOT NULL DEFAULT '[]',
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS assets (
                 id TEXT PRIMARY KEY,
                 project TEXT NOT NULL DEFAULT '',
                 archetype TEXT NOT NULL DEFAULT 'item_icon',
                 file_path TEXT NOT NULL UNIQUE,
                 prompt TEXT NOT NULL DEFAULT '',
                 model TEXT NOT NULL DEFAULT '',
                 via TEXT NOT NULL DEFAULT '',
                 width INTEGER NOT NULL DEFAULT 1024,
                 height INTEGER NOT NULL DEFAULT 1024,
                 created_at INTEGER NOT NULL
             );",
        )?;

        let _ = conn.execute(
            "ALTER TABLE projects ADD COLUMN reference_images_json TEXT NOT NULL DEFAULT '[]'",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE assets ADD COLUMN archetype TEXT NOT NULL DEFAULT 'item_icon'",
            [],
        );

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn sync_existing_outputs(&self, library_root: &Path) -> AppResult<()> {
        if !library_root.exists() {
            return Ok(());
        }
        let entries = fs::read_dir(library_root)?;
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                // Project folder
                let proj_json = path.join("project.json");
                let mut style = String::new();
                let mut style_prompt = String::new();
                let mut ref_imgs: Vec<String> = Vec::new();
                if let Ok(raw) = fs::read_to_string(&proj_json) {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                        style = v
                            .get("style")
                            .and_then(|s| s.as_str())
                            .unwrap_or("")
                            .to_string();
                        style_prompt = v
                            .get("stylePrompt")
                            .and_then(|s| s.as_str())
                            .unwrap_or("")
                            .to_string();
                        if let Some(arr) = v.get("referenceImages").and_then(|a| a.as_array()) {
                            ref_imgs = arr
                                .iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect();
                        }
                    }
                }
                let _ = self.upsert_project(&file_name, &style, &style_prompt, &ref_imgs);
                self.scan_dir_images(&path, &file_name)?;
            } else if Self::is_image_file(&path) {
                self.register_existing_image(&path, "")?;
            }
        }
        Ok(())
    }

    fn scan_dir_images(&self, dir: &Path, project: &str) -> AppResult<()> {
        for entry in fs::read_dir(dir)?.flatten() {
            let p = entry.path();
            if p.is_file() && Self::is_image_file(&p) {
                self.register_existing_image(&p, project)?;
            }
        }
        Ok(())
    }

    fn is_image_file(p: &Path) -> bool {
        matches!(
            p.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .as_deref(),
            Some("png" | "jpg" | "jpeg" | "webp")
        )
    }

    fn register_existing_image(&self, path: &Path, project: &str) -> AppResult<()> {
        let full = path.to_string_lossy().replace('\\', "/");
        let conn = self.conn.lock().unwrap();
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(1) FROM assets WHERE file_path = ?1",
                params![full],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if exists > 0 {
            return Ok(());
        }
        let mtime = fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
        let (w, h) = image::image_dimensions(path).unwrap_or((1024, 1024));
        let id = format!("ast_{}", uuid::Uuid::new_v4().simple());
        conn.execute(
            "INSERT OR IGNORE INTO assets (id, project, file_path, prompt, model, via, width, height, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, project, full, "", "imported", "local", w, h, mtime],
        )?;
        Ok(())
    }

    pub fn upsert_project(
        &self,
        name: &str,
        style: &str,
        style_prompt: &str,
        reference_images: &[String],
    ) -> AppResult<()> {
        let now = chrono::Utc::now().timestamp_millis();
        let ref_json = serde_json::to_string(reference_images).unwrap_or_else(|_| "[]".to_string());
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO projects (name, style, style_prompt, reference_images_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(name) DO UPDATE SET
               style = excluded.style,
               style_prompt = excluded.style_prompt,
               reference_images_json = excluded.reference_images_json",
            params![name, style, style_prompt, ref_json, now],
        )?;
        Ok(())
    }

    pub fn rename_project(&self, old_name: &str, new_name: &str, library_root: &Path) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE projects SET name = ?1 WHERE name = ?2",
            params![new_name, old_name],
        )?;

        let old_prefix = library_root
            .join(old_name)
            .to_string_lossy()
            .replace('\\', "/");
        let new_prefix = library_root
            .join(new_name)
            .to_string_lossy()
            .replace('\\', "/");

        let mut stmt = conn.prepare("SELECT id, file_path FROM assets WHERE project = ?1")?;
        let rows: Vec<(String, String)> = stmt
            .query_map(params![old_name], |r| Ok((r.get(0)?, r.get(1)?)))?
            .flatten()
            .collect();
        drop(stmt);

        for (id, fp) in rows {
            let updated_fp = if fp.starts_with(&old_prefix) {
                format!("{}{}", new_prefix, &fp[old_prefix.len()..])
            } else {
                fp
            };
            conn.execute(
                "UPDATE assets SET project = ?1, file_path = ?2 WHERE id = ?3",
                params![new_name, updated_fp, id],
            )?;
        }
        Ok(())
    }

    pub fn list_projects(&self) -> AppResult<Vec<ProjectInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.name, p.style, p.style_prompt, p.reference_images_json,
                    (SELECT COUNT(1) FROM assets a WHERE a.project = p.name) as cnt
             FROM projects p
             ORDER BY p.name ASC",
        )?;
        let items = stmt
            .query_map([], |row| {
                let ref_raw: String = row.get(3).unwrap_or_else(|_| "[]".to_string());
                let refs: Vec<String> = serde_json::from_str::<Vec<String>>(&ref_raw)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|p: &String| p.starts_with("data:") || PathBuf::from(p).exists())
                    .collect();
                Ok(ProjectInfo {
                    name: row.get(0)?,
                    style: row.get(1)?,
                    style_prompt: row.get(2)?,
                    reference_images: refs,
                    asset_count: row.get::<_, i64>(4)? as usize,
                })
            })?
            .flatten()
            .collect();
        Ok(items)
    }

    pub fn insert_asset(&self, item: &AssetItem) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let arch = if item.archetype.trim().is_empty() {
            "item_icon"
        } else {
            item.archetype.trim()
        };
        conn.execute(
            "INSERT OR REPLACE INTO assets (id, project, archetype, file_path, prompt, model, via, width, height, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                item.id,
                item.project,
                arch,
                item.file_path,
                item.prompt,
                item.model,
                item.via,
                item.width,
                item.height,
                item.created_at
            ],
        )?;
        Ok(())
    }

    pub fn list_assets(&self, project_filter: Option<&str>, limit: usize) -> AppResult<Vec<AssetItem>> {
        let conn = self.conn.lock().unwrap();
        let mut out = Vec::new();
        match project_filter {
            Some("_all") | None => {
                let mut stmt = conn.prepare(
                    "SELECT id, project, file_path, prompt, model, via, width, height, created_at, archetype
                     FROM assets ORDER BY created_at DESC LIMIT ?1",
                )?;
                let rows = stmt.query_map(params![limit as i64], Self::map_asset_row)?;
                for r in rows.flatten() {
                    if PathBuf::from(&r.file_path).exists() {
                        out.push(r);
                    }
                }
            }
            Some(proj) => {
                let mut stmt = conn.prepare(
                    "SELECT id, project, file_path, prompt, model, via, width, height, created_at, archetype
                     FROM assets WHERE project = ?1 ORDER BY created_at DESC LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![proj, limit as i64], Self::map_asset_row)?;
                for r in rows.flatten() {
                    if PathBuf::from(&r.file_path).exists() {
                        out.push(r);
                    }
                }
            }
        }
        Ok(out)
    }

    pub fn delete_asset(&self, id: &str) -> AppResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let fp: Option<String> = conn
            .query_row(
                "SELECT file_path FROM assets WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .ok();
        conn.execute("DELETE FROM assets WHERE id = ?1", params![id])?;
        Ok(fp)
    }

    pub fn update_asset_archetype(&self, id: &str, archetype: &str) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE assets SET archetype = ?1 WHERE id = ?2",
            params![archetype.trim(), id],
        )?;
        Ok(())
    }

    fn map_asset_row(row: &rusqlite::Row) -> rusqlite::Result<AssetItem> {
        Ok(AssetItem {
            id: row.get(0)?,
            project: row.get(1)?,
            file_path: row.get(2)?,
            prompt: row.get(3)?,
            model: row.get(4)?,
            via: row.get(5)?,
            width: row.get(6)?,
            height: row.get(7)?,
            created_at: row.get(8)?,
            archetype: row.get(9).unwrap_or_else(|_| "item_icon".to_string()),
        })
    }
}
