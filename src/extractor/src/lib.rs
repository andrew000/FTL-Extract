pub mod ftl;

#[cfg(test)]
mod tests {

    #[test]
    fn _test_extract() {
        // cargo run --release -- extract $CODE_PATH $OUT_PATH -l "en" -l "uk" -l "ru" -K "LF" -K "LazyProxy" -I "core" --comment-junks --comment-keys-mode=comment --line-endings=crlf --verbose
        // cargo run --release -- stub $CODE_PATH/locales/en $CODE_PATH/stub.pyi
    }
}
