use crate::types::{CheckUntranslatedResult, UntranslatedKey};
use std::collections::BTreeMap;

pub fn render_untranslated_txt(result: &CheckUntranslatedResult) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "total_untranslated: {}\n",
        result.untranslated.len()
    ));
    out.push_str(&format!(
        "checked_locales: {}\n",
        result.checked_locales.join(", ")
    ));
    out.push_str(&format!(
        "fully_translated_locales: {}\n\n",
        result.fully_translated_locales.join(", ")
    ));

    for item in &result.untranslated {
        out.push_str(&format!(
            "[{}] {}:{} key={}\n",
            item.locale,
            item.file_path.display(),
            item.line.unwrap_or(0),
            item.key
        ));
        for suggestion in &item.suggestions {
            out.push_str(&format!(
                "  suggestion({}): {}\n",
                suggestion.locale, suggestion.value
            ));
        }
    }

    out
}

pub fn render_untranslated_json(result: &CheckUntranslatedResult) -> String {
    let untranslated = result
        .untranslated
        .iter()
        .map(|item| {
            serde_json::json!({
                "locale": item.locale,
                "file_path": item.file_path.display().to_string(),
                "line": item.line,
                "key": item.key,
                "suggestions": item.suggestions.iter().map(|s| serde_json::json!({
                    "locale": s.locale,
                    "value": s.value
                })).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();

    serde_json::to_string_pretty(&serde_json::json!({
        "total_untranslated": result.untranslated.len(),
        "checked_locales": result.checked_locales,
        "fully_translated_locales": result.fully_translated_locales,
        "untranslated": untranslated
    }))
    .expect("Failed to serialize untranslated report")
}

pub fn render_untranslated_terminal(result: &CheckUntranslatedResult) -> String {
    if result.untranslated.is_empty() {
        return "No untranslated keys found.".to_string();
    }

    let mut by_locale: BTreeMap<&str, Vec<&UntranslatedKey>> = BTreeMap::new();
    for item in &result.untranslated {
        by_locale
            .entry(item.locale.as_str())
            .or_default()
            .push(item);
    }

    let mut out = String::new();
    for (locale, items) in &by_locale {
        out.push_str(&format!(
            "\nFound {} untranslated keys in {} locale:\n\n",
            items.len(),
            locale
        ));

        for (index, item) in items.iter().enumerate() {
            out.push_str(&format!(
                "{}. {}:{}\n   Key: {}\n",
                index + 1,
                item.file_path.display(),
                item.line.unwrap_or(0),
                item.key
            ));
            for suggestion in &item.suggestions {
                out.push_str(&format!(
                    "   Suggestion from '{}': {} = {}\n",
                    suggestion.locale, item.key, suggestion.value
                ));
            }
            out.push('\n');
        }
    }

    let locales_checked = by_locale
        .iter()
        .map(|(locale, items)| format!("{locale} ({})", items.len()))
        .collect::<Vec<_>>()
        .join(", ");

    out.push_str("Summary:\n");
    out.push_str(&format!(
        "- Total untranslated keys: {}\n",
        result.untranslated.len()
    ));
    out.push_str(&format!("- Locales checked: {}\n", locales_checked));
    if !result.fully_translated_locales.is_empty() {
        out.push_str(&format!(
            "- Locales fully translated: {}\n",
            result.fully_translated_locales.join(", ")
        ));
    }

    out.trim_end().to_string()
}
