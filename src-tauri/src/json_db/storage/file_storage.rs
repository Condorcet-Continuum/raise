// FICHIER : src-tauri/src/json_db/storage/file_storage.rs

use crate::json_db::storage::JsonDbConfig;
use crate::utils::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DropMode {
    Soft,
    Hard,
}

pub async fn open_db(config: &JsonDbConfig, space: &str, db: &str) -> RaiseResult<()> {
    let db_path = config.db_root(space, db);

    if !fs::exists_async(&db_path).await {
        raise_error!(
            "ERR_DB_FS_NOT_FOUND",
            error = format!(
                "Le répertoire de la base de données est introuvable : {}",
                db
            ),
            context = json_value!({
                "space": space,
                "db_name": db,
                "resolved_path": db_path,
                "action": "open_database_storage",
                "hint": "Si c'est un premier lancement, assurez-vous d'appeler 'create_db' avant 'open_db'."
            })
        );
    }

    Ok(())
}

/// Crée l'arborescence physique de la base de données.
pub async fn create_db(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    system_doc: &JsonValue,
) -> RaiseResult<bool> {
    let db_root = config.db_root(space, db);

    // 1. 🎯 OPTIMISATION ABSOLUE : Return Early
    // Si le dossier de la base existe, on ne fait STRICTEMENT rien.
    if fs::exists_async(&db_root).await {
        return Ok(false);
    }

    // 2. Création du dossier racine de la base
    fs::create_dir_all_async(&db_root).await?;

    let app_config = AppConfig::get();
    // 🎯 FIX : Utilisation stricte des points de montage système
    if space == app_config.mount_points.system.domain && db == app_config.mount_points.system.db {
        #[cfg(debug_assertions)]
        println!("🚀 Initialisation de la base SYSTEME détectée.");
    }

    // 3. INTROSPECTION DYNAMIQUE (Exécutée une seule fois à la naissance de la DB)
    if let Some(root_obj) = system_doc.as_object() {
        for (category, category_data) in root_obj {
            if let Some(sub_nodes) = category_data.as_object() {
                for (name, node_data) in sub_nodes {
                    if let Some(node_obj) = node_data.as_object() {
                        // Heuristique : Un nœud avec un tableau "items" = un dossier physique
                        if node_obj.get("items").is_some_and(|i| i.is_array()) {
                            let path = db_root.join(category).join(name);

                            // On crée l'arborescence (ex: /collections/actors)
                            if let Err(e) = fs::create_dir_all_async(&path).await {
                                raise_error!(
                                    "ERR_FS_DYNAMIC_DIR_FAILED",
                                    error = e,
                                    context = json_value!({
                                        "category": category,
                                        "name": name,
                                        "path": path.to_string_lossy().to_string()
                                    })
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(true)
}

pub async fn drop_db(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    mode: DropMode,
) -> RaiseResult<()> {
    let db_path = config.db_root(space, db);
    if !fs::exists_async(&db_path).await {
        return Ok(());
    }

    match mode {
        DropMode::Hard => {
            fs::remove_dir_all_async(&db_path).await?;
        }
        DropMode::Soft => {
            let timestamp = UtcClock::now().timestamp();
            let parent = match db_path.parent() {
                Some(p) => p,
                None => &db_path,
            };
            let new_name = format!("{}.deleted-{}", db, timestamp);
            let new_path = parent.join(new_name);

            fs::rename_async(&db_path, &new_path).await?;
        }
    }
    Ok(())
}

pub async fn write_document(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
    doc: &JsonValue,
) -> RaiseResult<()> {
    let col_path = config.db_collection_path(space, db, collection);
    fs::create_dir_all_async(&col_path).await?;
    let file_path = col_path.join(format!("{}.json", id));
    fs::write_json_atomic_async(&file_path, doc).await?;
    Ok(())
}

pub async fn read_document(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
) -> RaiseResult<Option<JsonValue>> {
    let file_path = config
        .db_collection_path(space, db, collection)
        .join(format!("{}.json", id));

    if !fs::exists_async(&file_path).await {
        return Ok(None);
    }

    let doc: JsonValue = fs::read_json_async(&file_path).await?;
    Ok(Some(doc))
}

pub async fn delete_document(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
) -> RaiseResult<()> {
    let file_path = config
        .db_collection_path(space, db, collection)
        .join(format!("{}.json", id));

    if fs::exists_async(&file_path).await {
        fs::remove_file_async(&file_path).await?;
    }
    Ok(())
}

pub async fn atomic_write<P: AsRef<Path>>(path: P, content: &[u8]) -> RaiseResult<()> {
    fs::write_atomic_async(path.as_ref(), content).await?;
    Ok(())
}

pub async fn atomic_write_binary<P: AsRef<Path>>(path: P, content: &[u8]) -> RaiseResult<()> {
    atomic_write(path, content).await
}

pub async fn save_database_index(path: &Path, data: &JsonValue) -> RaiseResult<()> {
    fs::write_json_atomic_async(path, data).await
}

pub async fn read_system_index(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
) -> RaiseResult<Option<JsonValue>> {
    let sys_path = config.db_root(space, db).join("_system.json");
    if fs::exists_async(&sys_path).await {
        let doc: JsonValue = fs::read_json_async(&sys_path).await?;
        Ok(Some(doc))
    } else {
        Ok(None)
    }
}

pub async fn write_system_index(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    doc: &JsonValue,
) -> RaiseResult<()> {
    let sys_path = config.db_root(space, db).join("_system.json");
    fs::write_json_atomic_async(&sys_path, doc).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::io::fs::tempdir;

    #[async_test]
    async fn test_atomic_write() -> RaiseResult<()> {
        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Échec création dossier temporaire : {:?}", e),
        };
        let file_path = dir.path().join("test.txt");

        atomic_write(&file_path, b"Hello World").await?;
        assert!(file_path.exists());

        let content = fs::read_to_string_async(&file_path).await?;
        assert_eq!(content, "Hello World");
        Ok(())
    }

    #[async_test]
    async fn test_document_lifecycle() -> RaiseResult<()> {
        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Échec création dossier temporaire : {:?}", e),
        };
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        let doc = json_value!({"name": "Refactor Test"});

        write_document(&config, "s1", "d1", "c1", "doc1", &doc).await?;

        let read = match read_document(&config, "s1", "d1", "c1", "doc1").await? {
            Some(d) => d,
            None => panic!("Document introuvable après écriture"),
        };
        assert_eq!(read["name"], "Refactor Test");

        let path = config
            .db_collection_path("s1", "d1", "c1")
            .join("doc1.json");
        assert!(fs::exists_async(&path).await);

        delete_document(&config, "s1", "d1", "c1", "doc1").await?;

        assert!(!fs::exists_async(&path).await);
        Ok(())
    }

    // 🎯 NOUVEAU TEST 1 : Introspection dynamique & Idempotence
    #[async_test]
    async fn test_create_db_dynamic_introspection() -> RaiseResult<()> {
        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Échec création dossier temporaire : {:?}", e),
        };
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let (space, db) = ("dyn_space", "dyn_db");

        // Un mock d'index hydraté avec des pièges (des objets sans "items")
        let system_doc = json_value!({
            "collections": {
                "users": { "items": [] },
                "posts": { "items": [] }
            },
            "rules": {
                "_system_rules": { "items": [] }
            },
            "schemas": {
                "v1": { "items": [] }
            },
            "fake_category": {
                "should_not_exist": { "foo": "bar" } // Ne devrait pas générer de dossier
            }
        });

        // 1. Première exécution : le dossier n'existe pas, il doit être créé
        let created = create_db(&config, space, db, &system_doc).await?;
        assert!(created, "La base aurait dû être créée");

        let db_root = config.db_root(space, db);

        // 2. Vérification de la création dynamique des chemins
        assert!(db_root.join("collections/users").exists());
        assert!(db_root.join("collections/posts").exists());
        assert!(db_root.join("rules/_system_rules").exists());
        assert!(db_root.join("schemas/v1").exists());

        // Ce dossier ne contient pas de tableau "items", il doit être ignoré
        assert!(!db_root.join("fake_category/should_not_exist").exists());

        // 3. Test de l'idempotence (Return Early)
        let created_again = create_db(&config, space, db, &system_doc).await?;
        assert!(
            !created_again,
            "Le Return Early a échoué, la base ne devrait pas être recréée"
        );
        Ok(())
    }

    // 🎯 NOUVEAU TEST 2 : Lecture et Écriture de l'Index Système
    #[async_test]
    async fn test_system_index_io() -> RaiseResult<()> {
        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Échec création dossier temporaire : {:?}", e),
        };
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let (space, db) = ("sys_space", "sys_db");

        // Lecture d'un index inexistant
        let none_index = read_system_index(&config, space, db).await?;
        assert!(none_index.is_none());

        // Création de la racine de la base pour pouvoir écrire dedans
        create_db(&config, space, db, &json_value!({})).await?;

        // Écriture d'un index
        let mock_doc = json_value!({ "name": "test_db", "version": 1 });
        write_system_index(&config, space, db, &mock_doc).await?;

        // Lecture et validation
        let read_index = match read_system_index(&config, space, db).await? {
            Some(idx) => idx,
            None => panic!("L'index devrait exister après écriture"),
        };
        assert_eq!(read_index["name"], "test_db");
        Ok(())
    }

    // 🎯 NOUVEAU TEST 3 : Comportement de open_db
    #[async_test]
    async fn test_open_db_validation() -> RaiseResult<()> {
        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Échec création dossier temporaire : {:?}", e),
        };
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let (space, db) = ("open_space", "open_db");

        // 1. Échec attendu car la base n'existe pas physiquement
        let res = open_db(&config, space, db).await;
        match res {
            Err(e) => assert!(e.to_string().contains("ERR_DB_FS_NOT_FOUND")),
            Ok(_) => panic!("La fonction open_db aurait dû échouer car la base n'existe pas"),
        }

        // 2. Succès après création
        create_db(&config, space, db, &json_value!({})).await?;
        let res_ok = open_db(&config, space, db).await;
        assert!(
            res_ok.is_ok(),
            "open_db devrait réussir sur une base existante"
        );
        Ok(())
    }
}
