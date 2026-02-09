// FICHIER : src-tauri/src/json_db/migrations/version.rs

use crate::utils::{
    error::AnyResult,
    fmt,                            // pub use std::fmt
    json::{Deserialize, Serialize}, //
    Ordering,                       // pub use std::cmp::Ordering
};

/// Représente une version de migration (Semantic Versioning simplifié)
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct MigrationVersion {
    major: u32,
    minor: u32,
    patch: u32,
    raw: String,
}

impl MigrationVersion {
    pub fn parse(version_str: &str) -> AnyResult<Self, String> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() != 3 {
            return Err(format!(
                "Format de version invalide (attendu x.y.z): {}",
                version_str
            ));
        }

        let major = parts[0].parse().map_err(|_| "Majeur non numérique")?;
        let minor = parts[1].parse().map_err(|_| "Mineur non numérique")?;
        let patch = parts[2].parse().map_err(|_| "Patch non numérique")?;

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
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
    }
}

impl PartialOrd for MigrationVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for MigrationVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    fn test_version_parsing() {
        let v = MigrationVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.raw, "1.2.3");

        assert!(MigrationVersion::parse("invalid").is_err());
        assert!(MigrationVersion::parse("1.2").is_err()); // Pas assez de parties
    }

    #[test]
    fn test_version_ordering() {
        let v1 = MigrationVersion::parse("1.0.0").unwrap();
        let v2 = MigrationVersion::parse("1.0.1").unwrap();
        let v3 = MigrationVersion::parse("1.1.0").unwrap();
        let v4 = MigrationVersion::parse("2.0.0").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);
        assert!(v1 < v4);
    }

    #[test]
    fn test_version_sorting_list() {
        let mut versions = vec![
            MigrationVersion::parse("2.0.0").unwrap(),
            MigrationVersion::parse("1.0.0").unwrap(),
            MigrationVersion::parse("1.5.0").unwrap(),
        ];
        versions.sort();

        assert_eq!(versions[0].raw, "1.0.0");
        assert_eq!(versions[1].raw, "1.5.0");
        assert_eq!(versions[2].raw, "2.0.0");
    }
}
