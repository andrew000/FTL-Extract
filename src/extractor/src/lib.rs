pub mod ftl;

#[cfg(test)]
mod tests {

    #[test]
    fn _test_extract() {
        // cargo run --release -- --silent --verbose extract $CODE_PATH $OUTPATH -l "en" -l "uk" -l "ru" -K "LF" -K "LazyProxy" -I "core" --comment-junks --comment-keys-mode=comment --line-endings=crlf
    }
}
