//! Local user profile stored in SQLite.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::avatar::Avatar;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub avatar: Avatar,
    #[serde(default)]
    pub headphones: bool,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub avatar_saved: bool,
}

fn default_kind() -> String {
    "bell".into()
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            name: String::new(),
            avatar: Avatar::default(),
            headphones: true,
            kind: default_kind(),
            avatar_saved: false,
        }
    }
}

impl Prefs {
    pub fn load() -> Self {
        Self::load_from(&db_path())
    }

    pub fn save(&self) {
        let _ = self.save_to(&db_path());
    }

    pub fn load_from(path: &Path) -> Self {
        match open_db(path) {
            Ok(conn) => match load_from_db(&conn) {
                Ok(prefs) => prefs,
                Err(_) => {
                    let migrated = migrate_json_prefs(path);
                    let _ = save_to_db(&conn, &migrated);
                    migrated
                }
            },
            Err(_) => migrate_json_prefs(path),
        }
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let conn = open_db(path)?;
        save_to_db(&conn, self)
    }
}

fn open_db(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let conn = Connection::open(path).map_err(|err| err.to_string())?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS user_profile (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            name TEXT NOT NULL DEFAULT '',
            avatar TEXT NOT NULL DEFAULT '',
            headphones INTEGER NOT NULL DEFAULT 1,
            kind TEXT NOT NULL DEFAULT 'bell',
            avatar_saved INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )
    .map_err(|err| err.to_string())?;
    Ok(conn)
}

fn load_from_db(conn: &Connection) -> Result<Prefs, String> {
    let mut stmt = conn
        .prepare(
            "SELECT name, avatar, headphones, kind, avatar_saved
             FROM user_profile WHERE id = 1",
        )
        .map_err(|err| err.to_string())?;
    let mut rows = stmt.query([]).map_err(|err| err.to_string())?;
    let Some(row) = rows.next().map_err(|err| err.to_string())? else {
        return Err("no profile row".into());
    };
    let name: String = row.get(0).map_err(|err| err.to_string())?;
    let avatar_raw: String = row.get(1).map_err(|err| err.to_string())?;
    let headphones: i64 = row.get(2).map_err(|err| err.to_string())?;
    let kind: String = row.get(3).map_err(|err| err.to_string())?;
    let avatar_saved: i64 = row.get(4).map_err(|err| err.to_string())?;
    Ok(Prefs {
        name,
        avatar: Avatar::decode(&avatar_raw).unwrap_or_default(),
        headphones: headphones != 0,
        kind: if kind.trim().is_empty() {
            default_kind()
        } else {
            kind
        },
        avatar_saved: avatar_saved != 0,
    })
}

fn save_to_db(conn: &Connection, prefs: &Prefs) -> Result<(), String> {
    conn.execute(
        "INSERT INTO user_profile (id, name, avatar, headphones, kind, avatar_saved, updated_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            avatar = excluded.avatar,
            headphones = excluded.headphones,
            kind = excluded.kind,
            avatar_saved = excluded.avatar_saved,
            updated_at = CURRENT_TIMESTAMP",
        params![
            prefs.name,
            prefs.avatar.encode(),
            if prefs.headphones { 1 } else { 0 },
            prefs.kind,
            if prefs.avatar_saved { 1 } else { 0 },
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

/// One-time import from the old `prefs.json` beside the database, if present.
fn migrate_json_prefs(db: &Path) -> Prefs {
    let path = legacy_json_beside(db);
    let Ok(bytes) = fs::read(&path) else {
        return Prefs::default();
    };
    let prefs = serde_json::from_slice::<Prefs>(&bytes).unwrap_or_default();
    let _ = fs::rename(&path, path.with_extension("json.bak"));
    prefs
}

fn data_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("DING_CONFIG_DIR") {
        return PathBuf::from(path);
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config/ding");
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("Ding");
    }
    PathBuf::from("ding-data")
}

fn db_path() -> PathBuf {
    if let Some(path) = std::env::var_os("DING_DB") {
        return PathBuf::from(path);
    }
    data_dir().join("ding.db")
}

fn legacy_json_beside(db: &Path) -> PathBuf {
    if let Some(path) = std::env::var_os("DING_CONFIG") {
        return PathBuf::from(path);
    }
    db.parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prefs.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avatar::{Accessory, BodyShape, Eyes, Mouth};

    fn temp_db(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ding-db-{label}-{}-{}",
            std::process::id(),
            label.len()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("ding.db")
    }

    #[test]
    fn round_trip_through_sqlite() {
        let path = temp_db("round");
        let prefs = Prefs {
            name: "Ada".into(),
            avatar: Avatar {
                shape: BodyShape::Bean,
                color: 3,
                eyes: Eyes::Wink,
                mouth: Mouth::Grin,
                accessory: Accessory::Sprout,
            },
            headphones: true,
            kind: "knock".into(),
            avatar_saved: true,
        };
        prefs.save_to(&path).unwrap();
        let loaded = Prefs::load_from(&path);
        assert_eq!(loaded.name, "Ada");
        assert_eq!(loaded.avatar, prefs.avatar);
        assert!(loaded.headphones);
        assert_eq!(loaded.kind, "knock");
        assert!(loaded.avatar_saved);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn migrates_legacy_json() {
        let path = temp_db("migrate");
        let dir = path.parent().unwrap();
        let legacy = Prefs {
            name: "Grace".into(),
            avatar: Avatar::default(),
            headphones: false,
            kind: "airhorn".into(),
            avatar_saved: true,
        };
        fs::write(
            dir.join("prefs.json"),
            serde_json::to_vec_pretty(&legacy).unwrap(),
        )
        .unwrap();

        let loaded = Prefs::load_from(&path);
        assert_eq!(loaded.name, "Grace");
        assert!(!loaded.headphones);
        assert_eq!(loaded.kind, "airhorn");
        assert!(path.exists());
        assert!(dir.join("prefs.json.bak").exists());
        let _ = fs::remove_dir_all(dir);
    }
}
