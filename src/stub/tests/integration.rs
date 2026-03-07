use std::fs;
use stub::{StubConfig, generate_stub};
use tempfile::TempDir;

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_generate_stub_integration() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let ftl_dir = temp_dir.path().join("ftl");
        let output_file = temp_dir.path().join("stub.pyi");

        fs::create_dir(&ftl_dir)?;

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

        let config = StubConfig {
            ftl_path: ftl_dir,
            output_path: output_file.clone(),
            export_tree: false,
        };

        generate_stub(config)?;

        assert!(output_file.exists());

        let content = fs::read_to_string(&output_file)?;

        assert!(content.contains("# mypy: ignore-errors"));
        assert!(content.contains("class I18nContext(I18nStub)"));
        assert!(content.contains("class LazyFactory(I18nStub)"));
        assert!(content.contains("class I18nStub:"));
        assert!(content.contains("def hello(*, name: Any, **kwargs: Any)"));
        assert!(content.contains("def goodbye(*, name: Any, **kwargs: Any)"));

        // chat-settings-timezone splits into chat → settings → timezone
        assert!(content.contains("class __Chat:"));
        assert!(content.contains("class __Settings:"));
        assert!(content.contains("def timezone(*, timezone: Any, **kwargs: Any)"));

        Ok(())
    }

    #[test]
    fn test_generate_stub_with_tree_export() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let ftl_dir = temp_dir.path().join("ftl");
        let output_file = temp_dir.path().join("stub.pyi");
        let json_file = temp_dir.path().join("stub.json");

        fs::create_dir(&ftl_dir)?;

        let test_ftl = "simple-message = Simple test message\n";
        fs::write(ftl_dir.join("test.ftl"), test_ftl)?;

        let config = StubConfig {
            ftl_path: ftl_dir,
            output_path: output_file.clone(),
            export_tree: true,
        };

        generate_stub(config)?;

        assert!(output_file.exists());
        assert!(json_file.exists());

        let json_content = fs::read_to_string(&json_file)?;
        let _parsed: serde_json::Value = serde_json::from_str(&json_content)?;

        Ok(())
    }

    #[test]
    fn test_generate_stub_empty_directory() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let ftl_dir = temp_dir.path().join("ftl");
        let output_file = temp_dir.path().join("stub.pyi");

        fs::create_dir(&ftl_dir)?;

        let config = StubConfig {
            ftl_path: ftl_dir,
            output_path: output_file.clone(),
            export_tree: false,
        };

        generate_stub(config)?;

        assert!(output_file.exists());

        let content = fs::read_to_string(&output_file)?;
        assert!(content.contains("class I18nStub:"));
        assert!(content.contains("pass"));

        Ok(())
    }
}
