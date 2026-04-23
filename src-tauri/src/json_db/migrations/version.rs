// FICHIER : src-tauri/src/json_db/migrations/version.rs

use crate::utils::prelude::*;

/// Représente une version de migration (Semantic Versioning simplifié)
#[derive(Debug, Clone, Eq, PartialEq, Serializable, Deserializable)]
pub struct MigrationVersion {
    major: u32,
    minor: u32,
    patch: u32,
    raw: String,
}

impl MigrationVersion {
    pub fn parse(version_str: &str) -> RaiseResult<Self> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() != 3 {
            raise_error!(
                "ERR_MIGRATION_VERSION_FORMAT_INVALID",
                error = format!("Le format de version '{}' est invalide.", version_str),
                context = json_value!({
                    "version_input": version_str,
                    "expected_format": "x.y.z",
                    "segments_found": parts.len(),
                    "action": "parse_migration_version"
                })
            );
        }
        let major: u32 = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => raise_error!(
                "ERR_VERSION_PARSE_MAJOR",
                context = json_value!({ "value": parts[0], "hint": "Le composant 'Major' de la version doit être un nombre entier." })
            ),
        };

        let minor: u32 = match parts[1].parse() {
            Ok(v) => v,
            Err(_) => raise_error!(
                "ERR_VERSION_PARSE_MINOR",
                context = json_value!({ "value": parts[1], "hint": "Le composant 'Minor' de la version doit être un nombre entier." })
            ),
        };

        let patch: u32 = match parts[2].parse() {
            Ok(v) => v,
            Err(_) => raise_error!(
                "ERR_VERSION_PARSE_PATCH",
                context = json_value!({ "value": parts[2], "hint": "Le composant 'Patch' de la version doit être un nombre entier." })
            ),
        };

        Ok(Self {
            major,
            minor,
            patch,
            raw: version_str.to_string(),
        })
    }
}

// Implémentation du tri pour ordonner les migrations
impl Ord for MigrationVersion {
    fn cmp(&self, other: &Self) -> FmtOrdering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
    }
}

impl PartialOrd for MigrationVersion {
    fn partial_cmp(&self, other: &Self) -> Option<FmtOrdering> {
        Some(self.cmp(other))
    }
}

impl FmtDisplay for MigrationVersion {
    fn fmt(&self, f: &mut FmtCursor<'_>) -> FmtResult {
        write!(f, "{}", self.raw)
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() -> RaiseResult<()> {
        let v = MigrationVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.raw, "1.2.3");

        assert!(MigrationVersion::parse("invalid").is_err());
        assert!(MigrationVersion::parse("1.2").is_err());

        Ok(())
    }

    #[test]
    fn test_version_ordering() -> RaiseResult<()> {
        let v1 = MigrationVersion::parse("1.0.0").unwrap();
        let v2 = MigrationVersion::parse("1.0.1").unwrap();
        let v3 = MigrationVersion::parse("1.1.0").unwrap();
        let v4 = MigrationVersion::parse("2.0.0").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);
        assert!(v1 < v4);

        Ok(())
    }

    #[test]
    fn test_version_sorting_list() -> RaiseResult<()> {
        let mut versions = vec![
            MigrationVersion::parse("2.0.0").unwrap(),
            MigrationVersion::parse("1.0.0").unwrap(),
            MigrationVersion::parse("1.5.0").unwrap(),
        ];
        versions.sort();

        assert_eq!(versions[0].raw, "1.0.0");
        assert_eq!(versions[1].raw, "1.5.0");
        assert_eq!(versions[2].raw, "2.0.0");

        Ok(())
    }
}
