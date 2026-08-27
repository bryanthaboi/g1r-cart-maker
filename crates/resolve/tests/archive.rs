use resolve::archive::{
    extract_zip, file_digests, list_entries, read_entry, safe_entry, verify, ArchiveCache,
    ArchiveError, Limits,
};
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::{FileOptions, SimpleFileOptions};
use zip::{CompressionMethod, ZipWriter};

const HELLO_SHA: &str = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
const HELLO_MD5: &str = "5eb63bbbe01eeed093cb22bb8f5acdc3";

fn stored() -> FileOptions<'static, ()> {
    SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
}

fn deflated() -> FileOptions<'static, ()> {
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
}

fn build(dir: &Path, name: &str, entries: &[(&str, &[u8], FileOptions<'static, ()>)]) -> PathBuf {
    let path = dir.join(name);
    let mut zip = ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (entry, body, options) in entries {
        zip.start_file(*entry, *options).unwrap();
        zip.write_all(body).unwrap();
    }
    zip.finish().unwrap();
    path
}

fn plain(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
    let owned: Vec<(&str, &[u8], FileOptions<'static, ()>)> = entries
        .iter()
        .map(|(entry, body)| (*entry, *body, deflated()))
        .collect();
    build(dir, name, &owned)
}

fn temp(tag: &str) -> tempdir::TempDir {
    tempdir::TempDir::new(tag).unwrap()
}

#[test]
fn safe_entry_accepts_plain_relative_paths() {
    assert_eq!(
        safe_entry("manifest.json").unwrap(),
        PathBuf::from("manifest.json")
    );
    assert_eq!(
        safe_entry("./mod/./options_schema.lua").unwrap(),
        PathBuf::from("mod/options_schema.lua")
    );
}

#[test]
fn safe_entry_refuses_every_escape() {
    for name in [
        "",
        "..",
        "../evil",
        "mod/../../evil",
        "/etc/passwd",
        "C:/Windows/system32",
        "c:evil",
        "mod\\evil",
        "mod/\0evil",
        "./",
    ] {
        let problem = safe_entry(name).unwrap_err();
        assert!(
            matches!(problem, ArchiveError::Unsafe(_)),
            "{name:?} slipped through"
        );
    }
}

#[test]
fn extract_writes_only_inside_the_destination() {
    let dir = temp("extract");
    let archive = plain(
        dir.path(),
        "mod.zip",
        &[("manifest.json", b"{}"), ("art/label.png", b"png")],
    );
    let dest = dir.path().join("out");
    let written = extract_zip(&archive, &dest, &Limits::default()).unwrap();
    assert_eq!(written.len(), 2);
    assert_eq!(
        std::fs::read_to_string(dest.join("manifest.json")).unwrap(),
        "{}"
    );
    assert!(dest.join("art/label.png").is_file());
}

#[test]
fn extract_refuses_a_traversal_entry() {
    let dir = temp("traversal");
    let archive = plain(dir.path(), "evil.zip", &[("../escaped.txt", b"nope")]);
    let dest = dir.path().join("out");
    let problem = extract_zip(&archive, &dest, &Limits::default()).unwrap_err();
    assert!(matches!(problem, ArchiveError::Unsafe(_)), "{problem}");
    assert!(!dir.path().join("escaped.txt").exists());
}

#[test]
fn extract_refuses_an_absolute_entry() {
    let dir = temp("absolute");
    let archive = plain(dir.path(), "evil.zip", &[("/tmp/escaped.txt", b"nope")]);
    let problem = extract_zip(&archive, &dir.path().join("out"), &Limits::default()).unwrap_err();
    assert!(matches!(problem, ArchiveError::Unsafe(_)), "{problem}");
}

#[test]
fn extract_refuses_a_drive_letter() {
    let dir = temp("drive");
    let archive = plain(dir.path(), "evil.zip", &[("C:/escaped.txt", b"nope")]);
    let problem = extract_zip(&archive, &dir.path().join("out"), &Limits::default()).unwrap_err();
    assert!(matches!(problem, ArchiveError::Unsafe(_)), "{problem}");
}

#[test]
fn extract_refuses_a_backslash_separator() {
    let dir = temp("backslash");
    let archive = plain(dir.path(), "evil.zip", &[("..\\escaped.txt", b"nope")]);
    let problem = extract_zip(&archive, &dir.path().join("out"), &Limits::default()).unwrap_err();
    assert!(matches!(problem, ArchiveError::Unsafe(_)), "{problem}");
}

#[test]
fn extract_refuses_a_symlink() {
    let dir = temp("symlink");
    let path = dir.path().join("link.zip");
    let mut zip = ZipWriter::new(std::fs::File::create(&path).unwrap());
    zip.add_symlink("link", "/etc/passwd", SimpleFileOptions::default())
        .unwrap();
    zip.finish().unwrap();
    let problem = extract_zip(&path, &dir.path().join("out"), &Limits::default()).unwrap_err();
    assert!(matches!(problem, ArchiveError::Symlink(_)), "{problem}");
}

#[test]
fn extract_refuses_too_many_entries() {
    let dir = temp("count");
    let names: Vec<String> = (0..20).map(|index| format!("f{}.txt", index)).collect();
    let entries: Vec<(&str, &[u8])> = names
        .iter()
        .map(|name| (name.as_str(), &b"x"[..]))
        .collect();
    let archive = plain(dir.path(), "many.zip", &entries);
    let limits = Limits {
        max_entries: 10,
        ..Limits::default()
    };
    let problem = extract_zip(&archive, &dir.path().join("out"), &limits).unwrap_err();
    assert!(
        matches!(
            problem,
            ArchiveError::TooManyEntries {
                count: 20,
                limit: 10
            }
        ),
        "{problem}"
    );
}

#[test]
fn extract_refuses_a_compression_ratio_bomb() {
    let dir = temp("ratio");
    let archive = build(
        dir.path(),
        "bomb.zip",
        &[("zeros.bin", &vec![0u8; 1_000_000], deflated())],
    );
    let limits = Limits {
        max_ratio: 10,
        ..Limits::default()
    };
    let problem = extract_zip(&archive, &dir.path().join("out"), &limits).unwrap_err();
    assert!(matches!(problem, ArchiveError::Ratio { .. }), "{problem}");
}

#[test]
fn extract_refuses_more_bytes_than_the_cap() {
    let dir = temp("bomb");
    let body: Vec<u8> = (0..200_000u32).map(|index| (index % 251) as u8).collect();
    let archive = build(dir.path(), "big.zip", &[("big.bin", &body, stored())]);
    let limits = Limits {
        max_total_bytes: 1024,
        ..Limits::default()
    };
    let dest = dir.path().join("out");
    let problem = extract_zip(&archive, &dest, &limits).unwrap_err();
    assert!(
        matches!(problem, ArchiveError::TooBig { limit: 1024 }),
        "{problem}"
    );
    assert!(!dest.join("big.bin").exists());
}

#[test]
fn one_file_reads_out_without_extracting() {
    let dir = temp("read");
    let archive = plain(
        dir.path(),
        "mod.zip",
        &[
            ("manifest.json", br#"{"id":"demo"}"#),
            ("options_schema.lua", b"return {}"),
        ],
    );
    assert_eq!(
        read_entry(&archive, "manifest.json", 1 << 20).unwrap(),
        br#"{"id":"demo"}"#
    );
    assert_eq!(
        read_entry(&archive, "./options_schema.lua", 1 << 20).unwrap(),
        b"return {}"
    );
    let problem = read_entry(&archive, "nope.json", 1 << 20).unwrap_err();
    assert!(matches!(problem, ArchiveError::Missing(_)), "{problem}");
    let problem = read_entry(&archive, "../manifest.json", 1 << 20).unwrap_err();
    assert!(matches!(problem, ArchiveError::Unsafe(_)), "{problem}");
}

#[test]
fn reading_one_file_honors_its_own_cap() {
    let dir = temp("readcap");
    let archive = plain(dir.path(), "mod.zip", &[("big.txt", &[b'x'; 4096])]);
    let problem = read_entry(&archive, "big.txt", 16).unwrap_err();
    assert!(
        matches!(problem, ArchiveError::TooBig { limit: 16 }),
        "{problem}"
    );
}

#[test]
fn a_file_that_is_not_a_zip_says_so() {
    let dir = temp("notazip");
    let path = dir.path().join("mod.zip");
    std::fs::write(&path, b"this is not a zip").unwrap();
    let problem = extract_zip(&path, &dir.path().join("out"), &Limits::default()).unwrap_err();
    assert!(matches!(problem, ArchiveError::NotAZip { .. }), "{problem}");
}

#[test]
fn entries_list_verbatim() {
    let dir = temp("list");
    let archive = plain(
        dir.path(),
        "mod.zip",
        &[("a.txt", b"a"), ("dir/b.txt", b"b")],
    );
    assert_eq!(list_entries(&archive).unwrap(), ["a.txt", "dir/b.txt"]);
}

#[test]
fn digests_match_the_known_values() {
    let dir = temp("digest");
    let path = dir.path().join("hello.bin");
    std::fs::write(&path, b"hello world").unwrap();
    let (sha, md5) = file_digests(&path).unwrap();
    assert_eq!(sha, HELLO_SHA);
    assert_eq!(md5, HELLO_MD5);
}

#[test]
fn verify_refuses_a_hash_that_does_not_match() {
    let dir = temp("verify");
    let path = dir.path().join("hello.bin");
    std::fs::write(&path, b"hello world").unwrap();
    verify(&path, Some(&HELLO_SHA.to_uppercase()), Some(HELLO_MD5)).unwrap();

    let wrong = "0".repeat(64);
    let problem = verify(&path, Some(&wrong), None).unwrap_err();
    match &problem {
        ArchiveError::HashMismatch { got, published, .. } => {
            assert_eq!(got, HELLO_SHA);
            assert_eq!(published, &wrong);
        }
        other => panic!("{other}"),
    }
    assert!(
        problem.to_string().contains("does not match the pin"),
        "{problem}"
    );

    let problem = verify(&path, None, Some(&"0".repeat(32))).unwrap_err();
    assert!(
        matches!(problem, ArchiveError::HashMismatch { .. }),
        "{problem}"
    );
}

#[test]
fn the_cache_is_keyed_by_sha256_and_refuses_a_liar() {
    let dir = temp("cache");
    let cache = ArchiveCache::new(dir.path().join("store"));
    assert!(cache.get(HELLO_SHA).is_none());

    let staged = dir.path().join("staged.zip");
    std::fs::write(&staged, b"hello world").unwrap();
    let stored = cache.store(&staged, HELLO_SHA).unwrap();
    assert!(stored.ends_with(format!("b9/{}.zip", HELLO_SHA)));
    assert_eq!(cache.get(HELLO_SHA).unwrap(), stored);
    assert_eq!(std::fs::read(&stored).unwrap(), b"hello world");

    let liar = dir.path().join("liar.zip");
    std::fs::write(&liar, b"not hello world").unwrap();
    let problem = cache.store(&liar, HELLO_SHA).unwrap_err();
    assert!(
        matches!(problem, ArchiveError::HashMismatch { .. }),
        "{problem}"
    );

    cache.forget(HELLO_SHA);
    assert!(cache.get(HELLO_SHA).is_none());
}
