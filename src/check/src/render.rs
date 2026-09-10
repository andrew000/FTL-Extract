use crate::types::{CheckResult, Diagnostic, DiagnosticKind, Severity};
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub fn render_check_json(result: &CheckResult) -> String {
    let diagnostics = result
        .diagnostics
        .iter()
        .map(|diagnostic| {
            serde_json::json!({
                "severity": diagnostic.severity.as_str(),
                "kind": diagnostic.kind.as_str(),
                "locale": diagnostic.locale,
                "key": diagnostic.key,
                "ftl_location": diagnostic.ftl_location.as_ref().map(|location| serde_json::json!({
                    "path": location.path.display().to_string(),
                    "line": location.line,
                    "column": location.column
                })),
                "code_location": diagnostic.code_location.as_ref().map(|location| serde_json::json!({
                    "path": location.path.display().to_string(),
                    "line": location.line,
                    "column": location.column
                })),
                "message": diagnostic.message,
                "suggestions": diagnostic.suggestions.iter().map(|suggestion| serde_json::json!({
                    "locale": suggestion.locale,
                    "value": suggestion.value
                })).collect::<Vec<_>>(),
                "missing_kwargs": diagnostic.missing_kwargs,
                "unused_kwargs": diagnostic.unused_kwargs
            })
        })
        .collect::<Vec<_>>();

    serde_json::to_string_pretty(&serde_json::json!({
        "ok": result.error_count() == 0,
        "summary": {
            "errors": result.error_count(),
            "warnings": result.warning_count(),
            "checks": check_summaries(result).into_iter().map(|summary| serde_json::json!({
                "kind": summary.kind.as_str(),
                "errors": summary.errors,
                "warnings": summary.warnings,
                "passed": summary.errors == 0 && summary.warnings == 0
            })).collect::<Vec<_>>()
        },
        "diagnostics": diagnostics
    }))
    .expect("Failed to serialize check report")
}

pub fn render_check_terminal(result: &CheckResult) -> String {
    if result.diagnostics.is_empty() {
        let mut out = "FTL check passed: no problems found.".to_string();
        let summaries = check_summaries(result);
        if !summaries.is_empty() {
            out.push_str("\n\nChecks:\n");
            for summary in summaries {
                let _ = writeln!(out, "- {}: passed", summary.kind.as_str());
            }
        }
        return out;
    }

    let mut by_locale: BTreeMap<&str, Vec<&Diagnostic>> = BTreeMap::new();
    for diagnostic in &result.diagnostics {
        by_locale
            .entry(diagnostic.locale.as_deref().unwrap_or("-"))
            .or_default()
            .push(diagnostic);
    }

    let problem_count = result.diagnostics.len();
    let noun = if problem_count == 1 {
        "problem"
    } else {
        "problems"
    };
    let mut out = format!("FTL check failed: {problem_count} {noun}\n");

    for (locale, diagnostics) in by_locale {
        let _ = writeln!(out, "\nLocale: {locale}");
        for diagnostic in diagnostics {
            out.push('\n');
            let _ = writeln!(
                out,
                "{}[{}]: {}",
                diagnostic.severity.as_str(),
                diagnostic.kind.as_str(),
                diagnostic.message
            );
            if let Some(location) = &diagnostic.ftl_location {
                let column = location
                    .column
                    .map(|column| format!(":{column}"))
                    .unwrap_or_default();
                if let Some(line) = location.line {
                    let _ = writeln!(
                        out,
                        "  file: {}:{}{}",
                        location.path.display(),
                        line,
                        column
                    );
                } else {
                    let _ = writeln!(out, "  file: {}", location.path.display());
                }
            }
            if let Some(location) = &diagnostic.code_location {
                let column = location
                    .column
                    .map(|column| format!(":{column}"))
                    .unwrap_or_default();
                if let Some(line) = location.line {
                    let _ = writeln!(
                        out,
                        "  code: {}:{}{}",
                        location.path.display(),
                        line,
                        column
                    );
                } else {
                    let _ = writeln!(out, "  code: {}", location.path.display());
                }
            }
            for suggestion in &diagnostic.suggestions {
                let key = diagnostic.key.as_deref().unwrap_or("");
                let _ = writeln!(
                    out,
                    "  suggestion[{}]: {} = {}",
                    suggestion.locale, key, suggestion.value
                );
            }
            if !diagnostic.missing_kwargs.is_empty() {
                let _ = writeln!(
                    out,
                    "  missing in code: {}",
                    diagnostic.missing_kwargs.join(", ")
                );
            }
            if !diagnostic.unused_kwargs.is_empty() {
                let _ = writeln!(
                    out,
                    "  unused in ftl: {}",
                    diagnostic.unused_kwargs.join(", ")
                );
            }
        }
    }

    out.push_str("\nSummary:\n");
    let _ = writeln!(out, "- Errors: {}", result.error_count());
    let _ = writeln!(out, "- Warnings: {}", result.warning_count());
    out.push_str("- Checks:\n");
    for summary in check_summaries(result) {
        let status = if summary.errors == 0 && summary.warnings == 0 {
            "passed".to_string()
        } else {
            format!(
                "failed ({} errors, {} warnings)",
                summary.errors, summary.warnings
            )
        };
        let _ = writeln!(out, "  - {}: {}", summary.kind.as_str(), status);
    }

    out
}

pub fn has_failing_diagnostics(result: &CheckResult, fail_on: &[Severity]) -> bool {
    result.diagnostics.iter().any(|diagnostic| {
        fail_on
            .iter()
            .any(|severity| severity_fails(diagnostic.severity, *severity))
    })
}

fn severity_fails(severity: Severity, fail_on: Severity) -> bool {
    match fail_on {
        Severity::Error => severity == Severity::Error,
        Severity::Warn => true,
    }
}

#[derive(Debug)]
struct CheckSummary {
    kind: DiagnosticKind,
    errors: usize,
    warnings: usize,
}

fn check_summaries(result: &CheckResult) -> Vec<CheckSummary> {
    let mut kinds = result.checked_kinds.clone();
    for diagnostic in &result.diagnostics {
        if !kinds.contains(&diagnostic.kind) {
            kinds.push(diagnostic.kind);
        }
    }
    kinds.sort_by_key(|kind| kind.as_str());
    kinds.dedup();

    kinds
        .into_iter()
        .map(|kind| {
            let errors = result
                .diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.kind == kind && diagnostic.severity == Severity::Error
                })
                .count();
            let warnings = result
                .diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic.kind == kind && diagnostic.severity == Severity::Warn
                })
                .count();
            CheckSummary {
                kind,
                errors,
                warnings,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Diagnostic, DiagnosticKind, SourceLocation, TranslationSuggestion};
    use std::path::PathBuf;

    fn result() -> CheckResult {
        CheckResult {
            checked_kinds: vec![DiagnosticKind::Untranslated, DiagnosticKind::Syntax],
            diagnostics: vec![Diagnostic {
                severity: Severity::Error,
                kind: DiagnosticKind::Untranslated,
                locale: Some("uk".to_string()),
                key: Some("welcome".to_string()),
                ftl_location: Some(SourceLocation {
                    path: PathBuf::from("locales/uk/_default.ftl"),
                    line: Some(1),
                    column: None,
                }),
                code_location: None,
                message: "key `welcome` is untranslated in locale `uk`".to_string(),
                suggestions: vec![TranslationSuggestion {
                    locale: "en".to_string(),
                    value: "Welcome".to_string(),
                }],
                missing_kwargs: Vec::new(),
                unused_kwargs: Vec::new(),
            }],
        }
    }

    #[test]
    fn test_render_check_terminal() {
        let rendered = render_check_terminal(&result());

        assert!(rendered.contains("FTL check failed: 1 problem"));
        assert!(rendered.contains("error[untranslated]"));
        assert!(rendered.contains("locales/uk/_default.ftl:1"));
        assert!(rendered.contains("suggestion[en]: welcome = Welcome"));
        assert!(rendered.contains("syntax: passed"));
        assert!(rendered.contains("untranslated: failed"));
    }

    #[test]
    fn test_render_check_json() {
        let rendered = render_check_json(&result());

        assert!(rendered.contains(r#""ok": false"#));
        assert!(rendered.contains(r#""kind": "untranslated""#));
        assert!(rendered.contains(r#""path": "locales/uk/_default.ftl""#));
        assert!(rendered.contains(r#""value": "Welcome""#));
        assert!(rendered.contains(r#""checks""#));
    }

    #[test]
    fn test_has_failing_diagnostics() {
        assert!(has_failing_diagnostics(&result(), &[Severity::Error]));
        assert!(has_failing_diagnostics(&result(), &[Severity::Warn]));
        assert!(!has_failing_diagnostics(&result(), &[]));
    }
}
