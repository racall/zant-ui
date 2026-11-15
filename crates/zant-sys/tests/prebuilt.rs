//! Tests for the prebuilt-native logic shared with `build.rs`.
//!
//! `build.rs` itself cannot have `#[test]`s, so the hash, the metadata format
//! and the source-list plumbing live in `prebuilt.rs` and are tested here
//! against standard SHA-256 vectors. The end-to-end behaviour (build the
//! archive, link it, fall back on a stale one) is covered by
//! `scripts/release/prebuilt-check.sh`, which needs a full native toolchain.

#[path = "../prebuilt.rs"]
mod prebuilt;

use std::fs;
use std::path::{Path, PathBuf};

use prebuilt::{Metadata, Sha256};

fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("zant-prebuilt-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

// ---------------------------------------------------------------------------
// SHA-256
// ---------------------------------------------------------------------------

#[test]
fn sha256_matches_standard_vectors() {
    // FIPS 180-4 / RFC 6234 test vectors.
    assert_eq!(
        prebuilt::sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        prebuilt::sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        prebuilt::sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn sha256_handles_arbitrary_chunking() {
    // The hash of "abc" must not depend on how update() splits the input: the
    // source hash streams files through 64 KiB buffers.
    let expected = prebuilt::sha256_hex(b"abc");
    for split in 0..=3 {
        let mut hasher = Sha256::new();
        hasher.update(&b"abc"[..split]);
        hasher.update(&b"abc"[split..]);
        let digest = hasher.finish();
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(hex, expected, "split at {split}");
    }
}

// ---------------------------------------------------------------------------
// Source list + source hash
// ---------------------------------------------------------------------------

#[test]
fn committed_source_lists_name_existing_files() {
    // The lists are canonical: build.rs compiles exactly these files and the
    // prebuilt metadata hashes exactly these files.
    let root = workspace_root();
    for os in ["macos", "windows"] {
        let sources = prebuilt::read_sources(&root, os).expect("read source list");
        assert!(sources.len() >= 8, "{os}: only {} sources", sources.len());
        for source in &sources {
            assert!(
                root.join(source).is_file(),
                "{os}: source list names {}, which does not exist",
                source
            );
        }
        // Every compiled source (not header) has the extension the compile
        // functions filter for; the lists must not drift from them.
        let compiled = sources
            .iter()
            .filter(|source| {
                [".cpp", ".c", ".mm", ".m"]
                    .iter()
                    .any(|ext| source.ends_with(ext))
            })
            .count();
        assert!(compiled >= 3, "{os}: only {compiled} compilable sources");
    }
}

#[test]
fn source_hash_covers_contents_and_paths() {
    let root = temp_dir("source-hash");
    fs::create_dir_all(root.join("native")).expect("create native dir");
    fs::write(root.join("native/a.c"), b"int a(void) { return 1; }\n").unwrap();
    fs::write(root.join("native/b.h"), b"int a(void);\n").unwrap();

    let sources = vec!["native/a.c".to_owned(), "native/b.h".to_owned()];
    let first = prebuilt::source_hash(&root, &sources).expect("hash");
    assert_eq!(
        first,
        prebuilt::source_hash(&root, &sources).expect("hash again")
    );

    // Content change is detected.
    fs::write(root.join("native/a.c"), b"int a(void) { return 2; }\n").unwrap();
    let changed = prebuilt::source_hash(&root, &sources).expect("hash after edit");
    assert_ne!(first, changed);

    // Renaming a file (same contents) is detected too: the path is hashed.
    fs::write(root.join("native/a.c"), b"int a(void) { return 1; }\n").unwrap();
    fs::rename(root.join("native/b.h"), root.join("native/c.h")).unwrap();
    let renamed = vec!["native/a.c".to_owned(), "native/c.h".to_owned()];
    assert_ne!(
        first,
        prebuilt::source_hash(&root, &renamed).expect("hash after rename")
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn source_hash_matches_sha256sum_style_input() {
    // The same bytes a shell pipeline would hash (see source_hash's doc
    // comment): path + "\n" + contents + "\n", in list order.
    let root = temp_dir("source-hash-bytes");
    fs::create_dir_all(root.join("native")).expect("create native dir");
    fs::write(root.join("native/a.c"), b"abc").unwrap();
    let sources = vec!["native/a.c".to_owned()];
    let mut expected = Vec::new();
    expected.extend_from_slice(b"native/a.c\n");
    expected.extend_from_slice(b"abc\n");
    assert_eq!(
        prebuilt::source_hash(&root, &sources).expect("hash"),
        prebuilt::sha256_hex(&expected)
    );
    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

fn metadata(source_hash: &str) -> Metadata {
    Metadata {
        format: prebuilt::FORMAT.to_owned(),
        platform: "macos".to_owned(),
        source_hash: source_hash.to_owned(),
        source_commit: "0123456789abcdef".to_owned(),
        toolchain: "Apple clang version 21.0.0".to_owned(),
        built_at: "1762000000".to_owned(),
    }
}

#[test]
fn metadata_round_trips() {
    let original = metadata("deadbeef");
    let encoded = original.encode();
    assert_eq!(Metadata::parse(&encoded).expect("parse"), original);
    // Comments and unknown keys are ignored (the format may grow).
    let extended = format!("# comment\n{encoded}future_key=value\n");
    assert_eq!(Metadata::parse(&extended).expect("parse"), original);
}

#[test]
fn metadata_requires_the_identity_fields() {
    for missing in ["format", "platform", "source_hash"] {
        let text = metadata("deadbeef")
            .encode()
            .lines()
            .filter(|line| !line.starts_with(&format!("{missing}=")))
            .collect::<Vec<_>>()
            .join("\n");
        let error = Metadata::parse(&text).expect_err("parse must fail");
        assert!(
            error.contains(missing),
            "error for a missing {missing} should name it: {error}"
        );
    }
    assert!(Metadata::parse("not a key=value line").is_err());
}

// ---------------------------------------------------------------------------
// Discovery + verification
// ---------------------------------------------------------------------------

/// Builds a fake workspace root with the real macOS source list and files, a
/// fake prebuilt archive and a metadata file matching it.
fn fake_tree(name: &str) -> (PathBuf, PathBuf) {
    let root = temp_dir(name);
    let list = root.join(prebuilt::SOURCES_MACOS);
    fs::create_dir_all(list.parent().expect("prebuilt dir")).unwrap();
    fs::create_dir_all(root.join("native")).unwrap();
    fs::write(root.join("native/a.cpp"), b"// a\n").unwrap();
    fs::write(root.join("native/b.h"), b"// b\n").unwrap();
    fs::write(&list, b"native/a.cpp\nnative/b.h\n").unwrap();

    let prebuilt_dir = root.join(prebuilt::PREBUILT_DIR);
    fs::create_dir_all(&prebuilt_dir).unwrap();
    fs::write(prebuilt_dir.join("libzant_native.a"), b"!<arch>\n").unwrap();
    (root, prebuilt_dir)
}

fn write_matching_metadata(root: &Path, prebuilt_dir: &Path, platform: &str) -> Metadata {
    let sources = prebuilt::read_sources(root, "macos").expect("sources");
    let mut meta = metadata(&prebuilt::source_hash(root, &sources).expect("hash"));
    meta.platform = platform.to_owned();
    fs::write(prebuilt::metadata_path(prebuilt_dir), meta.encode()).unwrap();
    meta
}

#[test]
fn matching_prebuilt_is_accepted() {
    let (root, prebuilt_dir) = fake_tree("verify-ok");
    let meta = write_matching_metadata(&root, &prebuilt_dir, "macos");
    let candidate = prebuilt::candidate_from(&root, "macos", None).expect("candidate");
    assert_eq!(candidate.source, prebuilt::Source::Default);
    let found = prebuilt::verify(&candidate, &root, "macos").expect("verify");
    assert_eq!(found.metadata, meta);
    assert_eq!(found.library, prebuilt_dir.join("libzant_native.a"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn stale_prebuilt_is_rejected_with_the_reason() {
    let (root, prebuilt_dir) = fake_tree("verify-stale");
    write_matching_metadata(&root, &prebuilt_dir, "macos");
    let candidate = prebuilt::candidate_from(&root, "macos", None).expect("candidate");

    // A changed native source must invalidate the archive.
    fs::write(root.join("native/a.cpp"), b"// a changed\n").unwrap();
    let error = prebuilt::verify(&candidate, &root, "macos").expect_err("stale archive");
    assert!(
        error.contains("native sources changed"),
        "unexpected error: {error}"
    );

    // A prebuilt for another platform is refused too.
    let (root, prebuilt_dir) = fake_tree("verify-platform");
    write_matching_metadata(&root, &prebuilt_dir, "windows");
    let candidate = prebuilt::candidate_from(&root, "macos", None).expect("candidate");
    let error = prebuilt::verify(&candidate, &root, "macos").expect_err("wrong platform");
    assert!(error.contains("windows"), "unexpected error: {error}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn missing_library_or_metadata_is_reported() {
    let (root, prebuilt_dir) = fake_tree("verify-missing");
    let candidate = prebuilt::candidate_from(&root, "macos", None).expect("candidate");

    // No metadata next to the archive.
    let error = prebuilt::verify(&candidate, &root, "macos").expect_err("no metadata");
    assert!(error.contains("metadata.txt"), "unexpected error: {error}");

    // Metadata but no archive.
    write_matching_metadata(&root, &prebuilt_dir, "macos");
    fs::remove_file(prebuilt_dir.join("libzant_native.a")).unwrap();
    let error = prebuilt::verify(&candidate, &root, "macos").expect_err("no archive");
    assert!(
        error.contains("does not exist"),
        "unexpected error: {error}"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn hash_and_library_names_are_per_platform() {
    assert_eq!(
        prebuilt::library_name("macos").expect("macos"),
        "libzant_native.a"
    );
    assert_eq!(
        prebuilt::library_name("windows").expect("windows"),
        "zant_native.lib"
    );
    assert!(prebuilt::library_name("linux").is_err());
    assert!(prebuilt::sources_file("linux").is_err());
}

#[test]
fn the_env_variable_names_the_directory_or_the_library() {
    let (root, prebuilt_dir) = fake_tree("verify-env");
    let dir = prebuilt_dir.to_str().expect("utf-8 path");
    let candidate = prebuilt::candidate_from(&root, "macos", Some(dir)).expect("candidate");
    assert_eq!(candidate.source, prebuilt::Source::Env);
    assert_eq!(candidate.library, prebuilt_dir.join("libzant_native.a"));
    let library = prebuilt_dir.join("libzant_native.a");
    let candidate = prebuilt::candidate_from(&root, "macos", library.to_str()).expect("candidate");
    assert_eq!(candidate.dir, prebuilt_dir);
    // blank means unset
    let candidate = prebuilt::candidate_from(&root, "macos", Some("  ")).expect("candidate");
    assert_eq!(candidate.source, prebuilt::Source::Default);
    let _ = fs::remove_dir_all(&root);
}
