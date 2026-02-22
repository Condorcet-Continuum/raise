// FICHIER : src-tauri/src/model_engine/sysml2/mod.rs

pub mod mapper;
pub mod parser;

pub use mapper::Sysml2ToArcadiaMapper;
pub use parser::{Rule, Sysml2Parser};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Vérifie qu'on a bien accès au Mapper et au Parseur depuis la racine du module
        let _mapper = Sysml2ToArcadiaMapper::new();
        let input = "package Empty {}";
        let _parsed = parser::parse_sysml_text(input);
        assert!(_parsed.is_ok());
    }
}
