use stub::{generate_stub, StubConfig};
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_generate_stub_integration() -> anyhow::Result<()> {
        // Create temporary directory for test
        let temp_dir = TempDir::new()?;
        let ftl_dir = temp_dir.path().join("ftl");
        let output_file = temp_dir.path().join("stub.pyi");

        // Create FTL directory
        fs::create_dir(&ftl_dir)?;

        // Create test FTL file
        let test_ftl = r#"# Test FTL file for integration testing
hello = Hello, { $name }!
goodbye = Goodbye, { $name }!

chat-settings-timezone = { $timezone }
chat-settings-unknown = Unknown setting

-warn-emoji = ⚠️
-confirm-emoji = ✅

test-term-reference = Check this: { -warn-emoji }
"#;
        fs::write(ftl_dir.join("test.ftl"), test_ftl)?;

        // Generate stub
        let config = StubConfig {
            ftl_path: ftl_dir,
            output_path: output_file.clone(),
            export_tree: false,
        };

        generate_stub(config)?;

        // Verify output file exists
        assert!(output_file.exists());

        // Read and verify content
        let content = fs::read_to_string(&output_file)?;

        // Check for basic structure
        assert!(content.contains("# mypy: ignore-errors"));
        assert!(content.contains("class I18nContext(I18nStub)"));
        assert!(content.contains("class LazyFactory(I18nStub)"));
        assert!(content.contains("class I18nStub:"));

        // Check for our test methods
        assert!(content.contains("def hello(*, name: Any, **kwargs: Any)"));
        assert!(content.contains("def goodbye(*, name: Any, **kwargs: Any)"));

        // Check for nested chat-settings structure (chat → settings → timezone)
        assert!(content.contains("class __Chat:"));
        assert!(content.contains("class __Settings:"));
        assert!(content.contains("def timezone(*, timezone: Any, **kwargs: Any)"));

        Ok(())
    }

    #[test]
    fn test_generate_stub_with_tree_export() -> anyhow::Result<()> {
        // Create temporary directory for test
        let temp_dir = TempDir::new()?;
        let ftl_dir = temp_dir.path().join("ftl");
        let output_file = temp_dir.path().join("stub.pyi");
        let json_file = temp_dir.path().join("stub.json");

        // Create FTL directory
        fs::create_dir(&ftl_dir)?;

        // Create simple test FTL file
        let test_ftl = "simple-message = Simple test message\n";
        fs::write(ftl_dir.join("test.ftl"), test_ftl)?;

        // Generate stub with tree export
        let config = StubConfig {
            ftl_path: ftl_dir,
            output_path: output_file.clone(),
            export_tree: true,
        };

        generate_stub(config)?;

        // Verify both files exist
        assert!(output_file.exists());
        assert!(json_file.exists());

        // Verify JSON content is valid
        let json_content = fs::read_to_string(&json_file)?;
        let _parsed: serde_json::Value = serde_json::from_str(&json_content)?;

        Ok(())
    }

    #[test]
    fn test_generate_stub_empty_directory() -> anyhow::Result<()> {
        // Create temporary directory for test
        let temp_dir = TempDir::new()?;
        let ftl_dir = temp_dir.path().join("ftl");
        let output_file = temp_dir.path().join("stub.pyi");

        // Create empty FTL directory
        fs::create_dir(&ftl_dir)?;

        // Generate stub
        let config = StubConfig {
            ftl_path: ftl_dir,
            output_path: output_file.clone(),
            export_tree: false,
        };

        generate_stub(config)?;

        // Verify output file exists
        assert!(output_file.exists());

        // Read and verify minimal content
        let content = fs::read_to_string(&output_file)?;
        assert!(content.contains("class I18nStub:"));
        assert!(content.contains("pass")); // Empty class should have pass

        Ok(())
    }
}
