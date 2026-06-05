use crate::parser::{CheckLocaleCache, LocatedEntry};
use std::path::Path;

#[derive(Debug)]
pub(super) struct LocaleResource<'a> {
    pub(super) path: &'a Path,
    pub(super) entries: &'a [LocatedEntry],
}

pub(super) fn read_locale_resources<'a>(
    cache: &'a CheckLocaleCache,
    locale: &str,
) -> Vec<LocaleResource<'a>> {
    cache
        .files(locale)
        .map(|file| LocaleResource {
            path: &file.relative_to_locales,
            entries: &file.entries,
        })
        .collect()
}
