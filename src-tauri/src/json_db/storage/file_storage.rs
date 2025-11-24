use super::JsonDbConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Index minimal de la DB (validé contre `db/index.schema.json` à l'étape suivante)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbIndex {
    pub version: u32,
    pub space: String,
    pub database: String,
    pub collections: std::collections::HashMap<String, DbCollection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCollection {
    pub schema: String, // ex: "actors/actor.schema.json"
    #[serde(default)]
    pub items: Vec<DbItemRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbItemRef {
    pub file: String, // ex: "actors/<uuid>.json"
}

#[derive(Debug, Clone, Copy)]
pub enum DropMode {
    Soft, // rename vers *.deleted-<ts>
    Hard, // suppression définitive
}

#[derive(Debug, Clone)]
pub struct DbHandle {
    pub space: String,
    pub database: String,
    pub root: PathBuf,
    pub index: DbIndex,
}

pub fn create_db(cfg: &JsonDbConfig, space: &str, db: &str) -> Result<DbHandle> {
    let space_root = cfg.space_root(space);
    fs::create_dir_all(&space_root)
        .with_context(|| format!("Création du dossier espace {}", space_root.display()))?;

    let db_root = cfg.db_root(space, db);
    let index_path = cfg.index_path(space, db);
    let schemas_root = cfg.db_schemas_root(space, db);

    // Sécurité: éviter d’écraser une DB existante
    if index_path.exists() {
        anyhow::bail!(
            "La base `{space}/{db}` existe déjà ({})",
            index_path.display()
        );
    }

    // Créer arborescence
    fs::create_dir_all(&db_root)
        .with_context(|| format!("Création du dossier DB {}", db_root.display()))?;
    // Préparer sous-dossiers utiles
    fs::create_dir_all(schemas_root)
        .with_context(|| "Création du dossier schemas/v1".to_string())?;

    // (Option) Seeder les schémas depuis le repo si vide
    seed_schemas_if_empty(cfg, space, db).ok();

    // Écrire l’index initial (vide)
    let idx = DbIndex {
        version: 1,
        space: space.to_string(),
        database: db.to_string(),
        collections: std::collections::HashMap::new(),
    };
    atomic_write_json(&index_path, &idx)
        .with_context(|| format!("Écriture index {}", index_path.display()))?;

    Ok(DbHandle {
        space: space.to_string(),
        database: db.to_string(),
        root: db_root,
        index: idx,
    })
}

pub fn open_db(cfg: &JsonDbConfig, space: &str, db: &str) -> Result<DbHandle> {
    let db_root = cfg.db_root(space, db);
    let index_path = cfg.index_path(space, db);

    if !index_path.exists() {
        anyhow::bail!(
            "Index introuvable pour `{space}/{db}` ({}). Avez-vous créé la DB ?",
            index_path.display()
        );
    }

    let data = fs::read_to_string(&index_path)
        .with_context(|| format!("Lecture index {}", index_path.display()))?;
    let idx: DbIndex =
        serde_json::from_str(&data).with_context(|| "Parse JSON de _system.json".to_string())?;

    Ok(DbHandle {
        space: space.to_string(),
        database: db.to_string(),
        root: db_root,
        index: idx,
    })
}

pub fn drop_db(cfg: &JsonDbConfig, space: &str, db: &str, mode: DropMode) -> Result<()> {
    let db_root = cfg.db_root(space, db);

    if !db_root.exists() {
        // Idempotent
        return Ok(());
    }

    match mode {
        DropMode::Soft => {
            let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let new_name = format!("{db}.deleted-{ts}");
            let target = cfg.space_root(space).join(new_name);
            fs::rename(&db_root, &target).with_context(|| {
                format!(
                    "Soft drop: renommage {} -> {}",
                    db_root.display(),
                    target.display()
                )
            })?;
        }
        DropMode::Hard => {
            fs::remove_dir_all(&db_root)
                .with_context(|| format!("Suppression {}", db_root.display()))?;
        }
    }
    Ok(())
}

/// Crée une collection dans la DB et met à jour l'index _system.json
pub fn create_collection(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    name: &str,
    schema_rel: &str, // ex: "actors/actor.schema.json"
) -> Result<()> {
    use anyhow::bail;

    // 0) Vérifier la présence de l’index (donc de la DB)
    let index_path = cfg.index_path(space, db);
    if !index_path.exists() {
        bail!("DB `{space}/{db}` introuvable. Crée-la d'abord.");
    }

    // 1) Vérifier que le schéma existe bien dans la DB (schemas/v1)
    let schema_path = cfg.db_schemas_root(space, db).join(schema_rel);
    if !schema_path.exists() {
        bail!("Schéma inexistant: {}", schema_path.display());
    }

    // 2) Charger l’index
    let data = std::fs::read_to_string(&index_path)
        .with_context(|| format!("Lecture {}", index_path.display()))?;
    let mut idx: DbIndex =
        serde_json::from_str(&data).with_context(|| "Parse JSON de _system.json".to_string())?;

    // 3) Unicité du nom de collection
    if idx.collections.contains_key(name) {
        bail!("La collection `{name}` existe déjà dans `{space}/{db}`");
    }

    // 4) Insérer la collection
    idx.collections.insert(
        name.to_string(),
        DbCollection {
            schema: schema_rel.to_string(),
            items: Vec::new(),
        },
    );

    // 5) Créer le dossier physique de la collection: <db_root>/collections/<name>
    let coll_dir = cfg.db_root(space, db).join("collections").join(name);
    std::fs::create_dir_all(&coll_dir)
        .with_context(|| format!("Création {}", coll_dir.display()))?;

    // 6) Écrire l’index mis à jour
    atomic_write_json(&index_path, &idx)?;

    Ok(())
}

/// Écriture atomique: .tmp puis rename
/// Écriture atomique: .tmp puis rename
pub fn atomic_write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).ok();

    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    {
        let mut f =
            fs::File::create(&tmp).with_context(|| format!("Création {}", tmp.display()))?;
        // Utilisation de to_string_pretty pour la lisibilité (ou to_string pour la perf)
        let s = serde_json::to_string_pretty(value)?;
        f.write_all(s.as_bytes())?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("Rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}
/// Copie récursive de `<repo>/schemas/v1` vers `<db>/schemas/v1` si ce dernier est vide
fn seed_schemas_if_empty(cfg: &JsonDbConfig, space: &str, db: &str) -> Result<()> {
    let dst = cfg.db_schemas_root(space, db);
    if dst
        .read_dir()
        .map(|mut it| it.next().is_none())
        .unwrap_or(true)
    {
        copy_dir_recursive(&cfg.schemas_dev_root, &dst)?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let sp = entry.path();
        let dp = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&sp, &dp)?;
        } else if ty.is_file() {
            fs::copy(&sp, &dp)
                .with_context(|| format!("Copie {} -> {}", sp.display(), dp.display()))?;
        }
    }
    Ok(())
}

/// Écriture atomique de données binaires (pour Bincode)
pub fn atomic_write_binary(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).ok();

    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    {
        let mut f =
            fs::File::create(&tmp).with_context(|| format!("Création {}", tmp.display()))?;
        f.write_all(data)?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("Rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}
