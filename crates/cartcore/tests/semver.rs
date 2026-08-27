// Expectations produced by running the reference src/mods/Semver.lua under luajit.

use cartcore::semver::{compare, satisfies, satisfies_reason, valid_range, Version};
use std::cmp::Ordering;

const CASES: &[(&str, &str, bool)] = &[
    ("1.2.3", "", true),
    ("1.2.3", "1.2.3", true),
    ("1.2.3", "=1.2.3", true),
    ("1.2.3", "==1.2.3", true),
    ("1.2.3", ">1.0.0", true),
    ("1.2.3", ">=1.0.0", true),
    ("1.2.3", "<2.0.0", true),
    ("1.2.3", "<=2", true),
    ("1.2.3", "^1.2", true),
    ("1.2.3", "^1.2.3", true),
    ("1.2.3", "^0.2", false),
    ("1.2.3", "^0.2.1", false),
    ("1.2.3", "^0.0.3", false),
    ("1.2.3", "^0", false),
    ("1.2.3", ">=1.0.0 <2.0.0", true),
    ("1.2.3", ">=1.0.0 <2.0.0 || >=3.0.0", true),
    ("1.2.3", "1 || 2", false),
    ("1.2.3", ">=1.0.0-beta", true),
    ("1.2.3", "^1.0.0-beta", true),
    ("1.2.3", "<=2.0.0||>=3", true),
    ("1.2.3", "^1", true),
    ("1.2.3", "<0.0.1", false),
    ("0.0.0", "", true),
    ("0.0.0", "1.2.3", false),
    ("0.0.0", "=1.2.3", false),
    ("0.0.0", "==1.2.3", false),
    ("0.0.0", ">1.0.0", false),
    ("0.0.0", ">=1.0.0", false),
    ("0.0.0", "<2.0.0", true),
    ("0.0.0", "<=2", true),
    ("0.0.0", "^1.2", false),
    ("0.0.0", "^1.2.3", false),
    ("0.0.0", "^0.2", false),
    ("0.0.0", "^0.2.1", false),
    ("0.0.0", "^0.0.3", false),
    ("0.0.0", "^0", true),
    ("0.0.0", ">=1.0.0 <2.0.0", false),
    ("0.0.0", ">=1.0.0 <2.0.0 || >=3.0.0", false),
    ("0.0.0", "1 || 2", false),
    ("0.0.0", ">=1.0.0-beta", false),
    ("0.0.0", "^1.0.0-beta", false),
    ("0.0.0", "<=2.0.0||>=3", true),
    ("0.0.0", "^1", false),
    ("0.0.0", "<0.0.1", true),
    ("1.2", "", true),
    ("1.2", "1.2.3", false),
    ("1.2", "=1.2.3", false),
    ("1.2", "==1.2.3", false),
    ("1.2", ">1.0.0", true),
    ("1.2", ">=1.0.0", true),
    ("1.2", "<2.0.0", true),
    ("1.2", "<=2", true),
    ("1.2", "^1.2", true),
    ("1.2", "^1.2.3", false),
    ("1.2", "^0.2", false),
    ("1.2", "^0.2.1", false),
    ("1.2", "^0.0.3", false),
    ("1.2", "^0", false),
    ("1.2", ">=1.0.0 <2.0.0", true),
    ("1.2", ">=1.0.0 <2.0.0 || >=3.0.0", true),
    ("1.2", "1 || 2", false),
    ("1.2", ">=1.0.0-beta", true),
    ("1.2", "^1.0.0-beta", true),
    ("1.2", "<=2.0.0||>=3", true),
    ("1.2", "^1", true),
    ("1.2", "<0.0.1", false),
    ("1.0.0-beta", "", true),
    ("1.0.0-beta", "1.2.3", false),
    ("1.0.0-beta", "=1.2.3", false),
    ("1.0.0-beta", "==1.2.3", false),
    ("1.0.0-beta", ">1.0.0", false),
    ("1.0.0-beta", ">=1.0.0", false),
    ("1.0.0-beta", "<2.0.0", true),
    ("1.0.0-beta", "<=2", true),
    ("1.0.0-beta", "^1.2", false),
    ("1.0.0-beta", "^1.2.3", false),
    ("1.0.0-beta", "^0.2", false),
    ("1.0.0-beta", "^0.2.1", false),
    ("1.0.0-beta", "^0.0.3", false),
    ("1.0.0-beta", "^0", false),
    ("1.0.0-beta", ">=1.0.0 <2.0.0", false),
    ("1.0.0-beta", ">=1.0.0 <2.0.0 || >=3.0.0", false),
    ("1.0.0-beta", "1 || 2", false),
    ("1.0.0-beta", ">=1.0.0-beta", true),
    ("1.0.0-beta", "^1.0.0-beta", true),
    ("1.0.0-beta", "<=2.0.0||>=3", true),
    ("1.0.0-beta", "^1", false),
    ("1.0.0-beta", "<0.0.1", false),
    ("1.0.0-beta.1", "", true),
    ("1.0.0-beta.1", "1.2.3", false),
    ("1.0.0-beta.1", "=1.2.3", false),
    ("1.0.0-beta.1", "==1.2.3", false),
    ("1.0.0-beta.1", ">1.0.0", false),
    ("1.0.0-beta.1", ">=1.0.0", false),
    ("1.0.0-beta.1", "<2.0.0", true),
    ("1.0.0-beta.1", "<=2", true),
    ("1.0.0-beta.1", "^1.2", false),
    ("1.0.0-beta.1", "^1.2.3", false),
    ("1.0.0-beta.1", "^0.2", false),
    ("1.0.0-beta.1", "^0.2.1", false),
    ("1.0.0-beta.1", "^0.0.3", false),
    ("1.0.0-beta.1", "^0", false),
    ("1.0.0-beta.1", ">=1.0.0 <2.0.0", false),
    ("1.0.0-beta.1", ">=1.0.0 <2.0.0 || >=3.0.0", false),
    ("1.0.0-beta.1", "1 || 2", false),
    ("1.0.0-beta.1", ">=1.0.0-beta", true),
    ("1.0.0-beta.1", "^1.0.0-beta", true),
    ("1.0.0-beta.1", "<=2.0.0||>=3", true),
    ("1.0.0-beta.1", "^1", false),
    ("1.0.0-beta.1", "<0.0.1", false),
    ("0.2.0", "", true),
    ("0.2.0", "1.2.3", false),
    ("0.2.0", "=1.2.3", false),
    ("0.2.0", "==1.2.3", false),
    ("0.2.0", ">1.0.0", false),
    ("0.2.0", ">=1.0.0", false),
    ("0.2.0", "<2.0.0", true),
    ("0.2.0", "<=2", true),
    ("0.2.0", "^1.2", false),
    ("0.2.0", "^1.2.3", false),
    ("0.2.0", "^0.2", true),
    ("0.2.0", "^0.2.1", false),
    ("0.2.0", "^0.0.3", false),
    ("0.2.0", "^0", false),
    ("0.2.0", ">=1.0.0 <2.0.0", false),
    ("0.2.0", ">=1.0.0 <2.0.0 || >=3.0.0", false),
    ("0.2.0", "1 || 2", false),
    ("0.2.0", ">=1.0.0-beta", false),
    ("0.2.0", "^1.0.0-beta", false),
    ("0.2.0", "<=2.0.0||>=3", true),
    ("0.2.0", "^1", false),
    ("0.2.0", "<0.0.1", false),
    ("0.0.3", "", true),
    ("0.0.3", "1.2.3", false),
    ("0.0.3", "=1.2.3", false),
    ("0.0.3", "==1.2.3", false),
    ("0.0.3", ">1.0.0", false),
    ("0.0.3", ">=1.0.0", false),
    ("0.0.3", "<2.0.0", true),
    ("0.0.3", "<=2", true),
    ("0.0.3", "^1.2", false),
    ("0.0.3", "^1.2.3", false),
    ("0.0.3", "^0.2", false),
    ("0.0.3", "^0.2.1", false),
    ("0.0.3", "^0.0.3", true),
    ("0.0.3", "^0", false),
    ("0.0.3", ">=1.0.0 <2.0.0", false),
    ("0.0.3", ">=1.0.0 <2.0.0 || >=3.0.0", false),
    ("0.0.3", "1 || 2", false),
    ("0.0.3", ">=1.0.0-beta", false),
    ("0.0.3", "^1.0.0-beta", false),
    ("0.0.3", "<=2.0.0||>=3", true),
    ("0.0.3", "^1", false),
    ("0.0.3", "<0.0.1", false),
    ("2.0.0", "", true),
    ("2.0.0", "1.2.3", false),
    ("2.0.0", "=1.2.3", false),
    ("2.0.0", "==1.2.3", false),
    ("2.0.0", ">1.0.0", true),
    ("2.0.0", ">=1.0.0", true),
    ("2.0.0", "<2.0.0", false),
    ("2.0.0", "<=2", true),
    ("2.0.0", "^1.2", false),
    ("2.0.0", "^1.2.3", false),
    ("2.0.0", "^0.2", false),
    ("2.0.0", "^0.2.1", false),
    ("2.0.0", "^0.0.3", false),
    ("2.0.0", "^0", false),
    ("2.0.0", ">=1.0.0 <2.0.0", false),
    ("2.0.0", ">=1.0.0 <2.0.0 || >=3.0.0", false),
    ("2.0.0", "1 || 2", true),
    ("2.0.0", ">=1.0.0-beta", true),
    ("2.0.0", "^1.0.0-beta", false),
    ("2.0.0", "<=2.0.0||>=3", true),
    ("2.0.0", "^1", false),
    ("2.0.0", "<0.0.1", false),
    ("3.4.5-alpha-1", "", true),
    ("3.4.5-alpha-1", "1.2.3", false),
    ("3.4.5-alpha-1", "=1.2.3", false),
    ("3.4.5-alpha-1", "==1.2.3", false),
    ("3.4.5-alpha-1", ">1.0.0", true),
    ("3.4.5-alpha-1", ">=1.0.0", true),
    ("3.4.5-alpha-1", "<2.0.0", false),
    ("3.4.5-alpha-1", "<=2", false),
    ("3.4.5-alpha-1", "^1.2", false),
    ("3.4.5-alpha-1", "^1.2.3", false),
    ("3.4.5-alpha-1", "^0.2", false),
    ("3.4.5-alpha-1", "^0.2.1", false),
    ("3.4.5-alpha-1", "^0.0.3", false),
    ("3.4.5-alpha-1", "^0", false),
    ("3.4.5-alpha-1", ">=1.0.0 <2.0.0", false),
    ("3.4.5-alpha-1", ">=1.0.0 <2.0.0 || >=3.0.0", true),
    ("3.4.5-alpha-1", "1 || 2", false),
    ("3.4.5-alpha-1", ">=1.0.0-beta", true),
    ("3.4.5-alpha-1", "^1.0.0-beta", false),
    ("3.4.5-alpha-1", "<=2.0.0||>=3", true),
    ("3.4.5-alpha-1", "^1", false),
    ("3.4.5-alpha-1", "<0.0.1", false),
];

#[test]
fn satisfies_table() {
    for (version, range, want) in CASES {
        let (got, err) = satisfies_reason(version, range);
        assert_eq!(err, None, "{:?} vs {:?}", version, range);
        assert_eq!(got, *want, "{:?} vs {:?}", version, range);
    }
}

#[test]
fn parses_every_component_shape() {
    type Parsed = Option<(u64, u64, u64, Option<&'static str>)>;
    let cases: &[(&str, Parsed)] = &[
        ("1.2.3", Some((1, 2, 3, None))),
        ("1", Some((1, 0, 0, None))),
        ("1.2", Some((1, 2, 0, None))),
        ("v1.2.3", Some((1, 2, 3, None))),
        ("V2", Some((2, 0, 0, None))),
        (" 1.0.0 ", Some((1, 0, 0, None))),
        ("1.0.0+build.5", Some((1, 0, 0, None))),
        ("1.0.0-rc.1+exp.sha", Some((1, 0, 0, Some("rc.1")))),
        ("1.2.3-0", Some((1, 2, 3, Some("0")))),
        ("01.02.03", Some((1, 2, 3, None))),
        ("3.4.5-alpha-1", Some((3, 4, 5, Some("alpha-1")))),
        // a trailing hyphen leaves an empty pre-release, which is no pre-release
        ("1.2.3-", Some((1, 2, 3, None))),
        ("", None),
        ("x", None),
        ("1.2.3.4", None),
        ("-1.0.0", None),
        ("1..2", None),
        (".1", None),
        ("1.", None),
        ("1.0.0-beta_1", None),
    ];
    for (text, want) in cases {
        let got = Version::parse(text);
        match want {
            None => assert!(got.is_none(), "{:?} should not parse", text),
            Some((major, minor, patch, pre)) => {
                let got = got.unwrap_or_else(|| panic!("{:?} should parse", text));
                assert_eq!(
                    (got.major, got.minor, got.patch, got.pre.as_deref()),
                    (*major, *minor, *patch, *pre),
                    "{:?}",
                    text
                );
            }
        }
    }
}

#[test]
fn prerelease_precedence() {
    let cases: &[(&str, &str, Option<Ordering>)] = &[
        ("1.0.0", "1.0.0", Some(Ordering::Equal)),
        ("1.0.0", "1.0.1", Some(Ordering::Less)),
        ("1.0.0-alpha", "1.0.0", Some(Ordering::Less)),
        ("1.0.0-alpha", "1.0.0-alpha.1", Some(Ordering::Less)),
        ("1.0.0-alpha.1", "1.0.0-alpha.beta", Some(Ordering::Less)),
        ("1.0.0-alpha.beta", "1.0.0-beta", Some(Ordering::Less)),
        ("1.0.0-beta", "1.0.0-beta.2", Some(Ordering::Less)),
        ("1.0.0-beta.2", "1.0.0-beta.11", Some(Ordering::Less)),
        ("1.0.0-beta.11", "1.0.0-rc.1", Some(Ordering::Less)),
        ("1.0.0-rc.1", "1.0.0", Some(Ordering::Less)),
        ("1.0.0-1", "1.0.0-a", Some(Ordering::Less)),
        ("1.0.0-01", "1.0.0-1", Some(Ordering::Equal)),
        ("1.0.0-1e2", "1.0.0-100", Some(Ordering::Equal)),
        ("1.0.0-.", "1.0.0-x", Some(Ordering::Less)),
        ("2", "1.9.9", Some(Ordering::Greater)),
        ("x", "1", None),
        ("1", "x", None),
    ];
    for (a, b, want) in cases {
        assert_eq!(compare(a, b), *want, "{:?} vs {:?}", a, b);
    }
    let mut sorted = ["1.0.0", "1.0.0-rc.1", "0.9.9", "1.0.0-alpha", "1.0.1"]
        .iter()
        .filter_map(|v| Version::parse(v))
        .collect::<Vec<_>>();
    sorted.sort();
    let shown: Vec<String> = sorted
        .iter()
        .map(|v| match &v.pre {
            Some(pre) => format!("{}.{}.{}-{}", v.major, v.minor, v.patch, pre),
            None => format!("{}.{}.{}", v.major, v.minor, v.patch),
        })
        .collect();
    assert_eq!(
        shown,
        ["0.9.9", "1.0.0-alpha", "1.0.0-rc.1", "1.0.0", "1.0.1"]
    );
}

#[test]
fn malformed_ranges_report_a_reason() {
    let cases: &[(&str, &str, &str)] = &[
        ("1.2.3", "  ", "empty range alternative"),
        ("1.2.3", "||", "empty range alternative"),
        ("1.2.3", ">>1.0.0", "unknown comparator \">>\" in range"),
        ("1.2.3", "=x", "unparsable version \"x\" in range"),
        ("1.2.3", "~1.0.0", "unparsable version \"~1.0.0\" in range"),
        ("1.2.3", ">= 1.0.0", "unparsable version \"\" in range"),
        (
            "1.2.3",
            "1.0.0 - 2.0.0",
            "unparsable version \"-\" in range",
        ),
        ("x", ">=1.0.0", "unparsable version \"x\""),
        ("x", "", "unparsable version \"x\""),
    ];
    for (version, range, want) in cases {
        let (ok, err) = satisfies_reason(version, range);
        assert!(!ok, "{:?} vs {:?}", version, range);
        assert_eq!(err.as_deref(), Some(*want), "{:?} vs {:?}", version, range);
    }
}

#[test]
fn alternatives_are_scanned_even_after_a_hit() {
    assert!(satisfies("1.2.3", "1.2.3 || 9.9.9"));
    let (ok, err) = satisfies_reason("1.2.3", "1.2.3 || >>9");
    assert!(!ok);
    assert_eq!(err.as_deref(), Some("unknown comparator \">>\" in range"));
}

#[test]
fn valid_range_is_grammar_only() {
    assert!(valid_range("").is_ok());
    assert!(valid_range(">=1.0.0 <2.0.0 || ^3").is_ok());
    assert!(valid_range("9.9.9").is_ok());
    assert_eq!(
        valid_range("~1").err().as_deref(),
        Some("unparsable version \"~1\" in range")
    );
    assert!(valid_range("||").is_err());
}
