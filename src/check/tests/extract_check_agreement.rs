//! `ftl check` and `ftl extract` decide with the same collector which variables a stored message
//! needs. Whenever the `kwargs` check is clean, `extract` must leave every message alone.
//!
//! Fixtures are written in the serializer's canonical form (no blank lines between entries) so
//! that an unchanged catalogue is byte-for-byte unchanged after `extract`.

use check::{CheckKwargsConfig, check_kwargs};
use extractor::ftl::consts::{
    CommentsKeyModes, DEFAULT_EXCLUDE_DIRS, DEFAULT_FTL_FILENAME, DEFAULT_I18N_KEYS,
    DEFAULT_IGNORE_ATTRIBUTES, DEFAULT_IGNORE_KWARGS, LineEndings,
};
use extractor::ftl::ftl_extractor::{ExtractConfig, extract};
use extractor::ftl::utils::FastHashSet;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

struct Fixture {
    name: &'static str,
    code: &'static str,
    ftl: &'static str,
}

/// Cases where `check` reports no kwargs problem. `extract` must not touch them.
const AGREED: &[Fixture] = &[
    Fixture {
        name: "plain variables",
        code: "i18n.hello(name=user.name, count=3)\n",
        ftl: "hello = Hello { $name }, { $count } items\n",
    },
    Fixture {
        name: "function positional argument",
        code: "i18n.items(count=5)\n",
        ftl: "items = You have { NUMBER($count) } items\n",
    },
    Fixture {
        name: "function in selector only",
        code: "i18n.items(count=5)\n",
        ftl: "items =\n    { NUMBER($count) ->\n        [one] One item\n       *[other] Many items\n    }\n",
    },
    Fixture {
        name: "function with literal named argument and nested placeable",
        code: "i18n.updated(created_at=now, amount=1)\n",
        ftl: "updated = { DATETIME($created_at, month: \"long\") } {{ $amount }}\n",
    },
    Fixture {
        name: "term with parameter",
        code: "i18n.about()\n",
        ftl: "-brand =\n    { $case ->\n        [gen] Bota\n       *[nom] Bot\n    }\nabout = Pro { -brand(case: \"gen\") }\n",
    },
    Fixture {
        name: "term without arguments",
        code: "i18n.about()\n",
        ftl: "-brand =\n    { $case ->\n        [gen] Bota\n       *[nom] Bot\n    }\nabout = Pro { -brand }\n",
    },
    Fixture {
        name: "term whose variable is never bound",
        code: "i18n.about()\n",
        ftl: "-brand = Bot { $suffix }\nabout = About { -brand(case: \"gen\") }\n",
    },
    Fixture {
        name: "term parameter next to a caller variable of the same name",
        code: "i18n.about(case=1)\n",
        ftl: "-brand =\n    { $case ->\n        [gen] Bota\n       *[nom] Bot\n    }\nabout = Pro { -brand(case: \"gen\") } ({ $case })\n",
    },
    Fixture {
        name: "recursive terms with literal bindings",
        code: "i18n.title()\n",
        ftl: "title = { -outer(case: \"genitive\") }\n-outer = { -inner(case: \"genitive\") }\n-inner = { -outer(case: \"genitive\") } { $case }\n",
    },
    Fixture {
        name: "message referenced from a term and from the caller",
        code: "i18n.title(case=1)\ni18n.subtitle(case=1)\ni18n.wrapper(case=1)\n",
        ftl: "-brand = { subtitle }\nsubtitle = Brand { $case }\nwrapper = { subtitle }\ntitle = { -brand(case: \"genitive\") } { wrapper }\n",
    },
    Fixture {
        name: "attribute reference",
        code: "i18n.a(x=1)\ni18n.b(x=1)\n",
        ftl: "b = B { $x }\n    .title = Title { $x }\na = See { b.title }\n",
    },
    Fixture {
        name: "attribute reference to a message not called from code",
        code: "i18n.a(x=1)\n",
        ftl: "b = B\n    .title = Title { $x }\na = See { b.title }\n",
    },
    Fixture {
        // `a` needs nothing although `b.title` uses `$x`.
        name: "message reference ignores the referenced message's attributes",
        code: "i18n.a()\n",
        ftl: "b = B\n    .title = Title { $x }\na = See { b }\n",
    },
    Fixture {
        // `i18n.b()` renders only the value of `b`, so `$x` in its own attribute is not needed.
        name: "own attribute variable is not required",
        code: "i18n.b()\n",
        ftl: "b = B\n    .title = Title { $x }\n",
    },
    Fixture {
        name: "own attribute variable next to the same variable in the value",
        code: "i18n.b(x=1)\n",
        ftl: "b = B { $x }\n    .title = Title { $x }\n",
    },
    Fixture {
        // `**data` can pass `name`; neither command can verify the key, so both leave it alone.
        name: "kwargs unpacked with **data",
        code: "def f(i18n, data): i18n.welcome(**data)\n",
        ftl: "welcome = Welcome, { $name }!\n",
    },
    Fixture {
        name: "message chain",
        code: "i18n.a(x=1, y=2, z=3)\ni18n.b(y=2, z=3)\ni18n.c(z=3)\n",
        ftl: "c = C { $z }\nb = B { $y } { c }\na = A { $x } { b }\n",
    },
    Fixture {
        name: "message reference cycle",
        code: "i18n.a(x=1, y=2)\ni18n.b(x=1, y=2)\n",
        ftl: "a = { $x } { b }\nb = { $y } { a }\n",
    },
    Fixture {
        // The dynamic key is never called by name; `# ftl-extract: ignore stale` keeps it in
        // both commands.
        name: "uncalled key marked ignore stale",
        code: "i18n.get(f\"status-{kind}\")\ni18n.hello()\n",
        ftl: "hello = Hello\n# ftl-extract: ignore stale\nstatus-ok = OK\n",
    },
];

fn write_fixture(fixture: &Fixture) -> TempDir {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("code")).unwrap();
    fs::create_dir_all(temp.path().join("locales/en")).unwrap();
    fs::write(temp.path().join("code/app.py"), fixture.code).unwrap();
    fs::write(temp.path().join("locales/en/_default.ftl"), fixture.ftl).unwrap();
    temp
}

fn check_config(temp: &TempDir) -> CheckKwargsConfig {
    CheckKwargsConfig {
        locales_path: temp.path().join("locales"),
        code_path: temp.path().join("code"),
        locales: vec!["en".to_string()],
        i18n_keys: DEFAULT_I18N_KEYS.clone(),
        i18n_keys_prefix: FastHashSet::default(),
        exclude_dirs: DEFAULT_EXCLUDE_DIRS.clone(),
        ignore_attributes: DEFAULT_IGNORE_ATTRIBUTES.clone(),
        ignore_kwargs: DEFAULT_IGNORE_KWARGS.clone(),
        default_ftl_file: PathBuf::from(DEFAULT_FTL_FILENAME),
        cache: false,
        cache_path: None,
        clear_cache: false,
    }
}

fn extract_config(temp: &TempDir) -> ExtractConfig {
    ExtractConfig {
        code_path: temp.path().join("code"),
        locales_path: temp.path().join("locales"),
        languages: vec!["en".to_string()],
        i18n_keys: DEFAULT_I18N_KEYS.clone(),
        i18n_keys_prefix: FastHashSet::default(),
        exclude_dirs: DEFAULT_EXCLUDE_DIRS.clone(),
        ignore_attributes: DEFAULT_IGNORE_ATTRIBUTES.clone(),
        ignore_kwargs: DEFAULT_IGNORE_KWARGS.clone(),
        default_ftl_file: PathBuf::from(DEFAULT_FTL_FILENAME),
        comment_keys_mode: CommentsKeyModes::Comment,
        line_endings: LineEndings::LF,
        dry_run: false,
        cache: false,
        cache_path: None,
        clear_cache: false,
        allow_parse_errors: false,
    }
}

#[test]
fn extract_leaves_every_message_alone_when_check_is_clean() {
    for fixture in AGREED {
        let temp = write_fixture(fixture);

        let result = check_kwargs(check_config(&temp)).unwrap();
        assert!(
            result.mismatches.is_empty(),
            "[{}] check reported {:?}",
            fixture.name,
            result.mismatches
        );
        assert!(result.extraction_errors.is_empty(), "[{}]", fixture.name);

        let stats = extract(extract_config(&temp)).unwrap();
        assert_eq!(
            stats.ftl_keys_commented["en"], 0,
            "[{}] commented",
            fixture.name
        );
        assert_eq!(
            stats.ftl_keys_updated["en"], 0,
            "[{}] updated",
            fixture.name
        );
        assert_eq!(stats.ftl_keys_added["en"], 0, "[{}] added", fixture.name);

        let after = fs::read_to_string(temp.path().join("locales/en/_default.ftl")).unwrap();
        assert_eq!(after, fixture.ftl, "[{}] file rewritten", fixture.name);
    }
}

struct Mismatch {
    fixture: Fixture,
    missing: &'static [&'static str],
    unused: &'static [&'static str],
    /// The file after `extract`: the original entry commented out, then the placeholder.
    rewritten: &'static str,
}

#[test]
fn extract_and_check_both_flag_a_real_mismatch() {
    // Control: the agreement is not vacuous. Both commands flag a missing variable, and both
    // flag a keyword argument that matches only a variable in the called message's own
    // attribute (the value is what `i18n.b(x=1)` renders, and it does not use `$x`).
    let cases = [
        Mismatch {
            fixture: Fixture {
                name: "missing variable",
                code: "i18n.items()\n",
                ftl: "items = You have { NUMBER($count) } items\n",
            },
            missing: &["count"],
            unused: &[],
            rewritten: "# items = You have { NUMBER($count) } items\n\nitems = items\n",
        },
        Mismatch {
            fixture: Fixture {
                name: "kwarg matching only an own attribute",
                code: "i18n.b(x=1)\n",
                ftl: "b = B\n    .title = Title { $x }\n",
            },
            missing: &[],
            unused: &["x"],
            rewritten: "# b = B\n#     .title = Title { $x }\n\nb = b{ $x }\n",
        },
    ];

    for case in cases {
        let temp = write_fixture(&case.fixture);

        let result = check_kwargs(check_config(&temp)).unwrap();
        assert_eq!(result.mismatches.len(), 1, "[{}]", case.fixture.name);
        assert_eq!(
            result.mismatches[0].missing_kwargs, case.missing,
            "[{}]",
            case.fixture.name
        );
        assert_eq!(
            result.mismatches[0].unused_kwargs, case.unused,
            "[{}]",
            case.fixture.name
        );

        let stats = extract(extract_config(&temp)).unwrap();
        assert_eq!(stats.ftl_keys_commented["en"], 1, "[{}]", case.fixture.name);
        assert_eq!(
            fs::read_to_string(temp.path().join("locales/en/_default.ftl")).unwrap(),
            case.rewritten,
            "[{}]",
            case.fixture.name
        );
    }
}
