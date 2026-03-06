pub mod fluent;
pub mod generator;
pub mod tree;

use anyhow::Result;
use std::path::PathBuf;

/// Configuration for stub generation
#[derive(Debug, Clone)]
pub struct StubConfig {
    /// Path to FTL files directory
    pub ftl_path: PathBuf,
    /// Output path for the .pyi stub file
    pub output_path: PathBuf,
    /// Whether to export intermediate tree structure as JSON
    pub export_tree: bool,
}

/// Generate Python type stub (.pyi) file from FTL files
pub fn generate_stub(config: StubConfig) -> Result<()> {
    log::info!(
        "Generating stub from FTL files at {}",
        config.ftl_path.display()
    );
    log::info!("Output will be written to {}", config.output_path.display());

    // Parse FTL files and extract messages
    let messages = fluent::parse_ftl_files(&config.ftl_path)?;
    log::debug!("Extracted {} messages from FTL files", messages.len());

    // Build nested tree structure from flat message keys
    let tree = tree::build_tree(messages)?;
    log::debug!("Built tree structure with {} top-level keys", tree.len());

    // Export tree as JSON if requested
    if config.export_tree {
        let tree_path = config.output_path.with_extension("json");
        tree::export_tree_json(&tree, &tree_path)?;
        log::info!("Exported tree structure to {}", tree_path.display());
    }

    // Generate Python stub content
    let stub_content = generator::generate_stub_content(&tree)?;
    log::debug!(
        "Generated {} characters of stub content",
        stub_content.len()
    );

    // Write to output file
    std::fs::write(&config.output_path, stub_content)?;
    log::info!(
        "Successfully wrote stub file to {}",
        config.output_path.display()
    );

    Ok(())
}
