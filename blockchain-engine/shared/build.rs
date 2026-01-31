fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Nouvelle syntaxe recommandée pour tonic-build 0.12+
    tonic_build::configure().compile_protos(&["protos/chaincode.proto"], &["protos"])?;
    Ok(())
}
