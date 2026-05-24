mod kwargs;
mod missing;
mod references;
mod stale;
mod syntax;
mod untranslated;

pub use kwargs::check_kwargs;
pub use missing::check_missing;
pub use references::check_references;
pub use stale::check_stale;
pub use syntax::check_syntax;
pub use untranslated::check_untranslated;

use anyhow::{Result, bail};
use globset::{Glob, GlobSetBuilder};
use std::path::Path;

pub(super) fn validate_locales(
    locales_path: &Path,
    available_locales: &[String],
    locales: &[String],
) -> Result<()> {
    if locales.is_empty() {
        bail!("Missing languages. Pass --language or set tool.ftl-extract.check.languages");
    }

    for locale in locales {
        if !available_locales.iter().any(|existing| existing == locale) {
            bail!(
                "Locale `{}` does not exist in `{}`",
                locale,
                locales_path.display()
            );
        }
    }

    Ok(())
}

pub(super) fn build_ignore_set(
    exclude_dirs: &extractor::ftl::utils::FastHashSet<String>,
) -> Result<globset::GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for exclude in exclude_dirs {
        builder.add(Glob::new(exclude.as_str())?);
    }
    Ok(builder.build()?)
}
