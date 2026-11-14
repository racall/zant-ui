//! Shared logic for the prebuilt native library.
//!
//! `build.rs` normally compiles the native C / C++ / Objective-C layer
//! (`crates/zant-sys/native/skia/*.cpp`, `native/platform/*`, …) on
//! every clean build. The release archives ship the result of that compile as
//! `third_party/zant-native-prebuilt/`, so an installed app only has to *link*
//! it. Both sides of that contract live here so they cannot drift:
//!
//! - the canonical source list (`prebuilt/sources-{macos,windows}.txt`), which
//!   is also what `build.rs` compiles and watches with `rerun-if-changed`;
//! - a SHA-256 over that list's file contents (`source_hash`), recorded in the
//!   archive's `metadata.txt` when the archive is built and recomputed from the
//!   tree before the archive is linked;
//! - the metadata format (`key=value` lines, no dependencies: `build.rs` has no
//!   access to serde).
//!
//! This file is compiled twice: by `build.rs` (`mod prebuilt;`) and by
//! `tests/prebuilt.rs` (`#[path = "../prebuilt.rs"]`), which pins the hash to
//! standard SHA-256 test vectors and the metadata parser to the format below
//! without needing a native build.
//!
//! `metadata.txt`, next to `libzant_native.a` / `zant_native.lib`:
//!
//! ```text
//! format=1
//! platform=macos
//! source_hash=<sha256 of the source list contents, see source_hash()>
//! source_commit=<git commit the archive was built from>
//! toolchain=<compiler that built it / Rust toolchain of the builder>
//! built_at=<unix seconds>
//! ```
//!
//! See `crates/zant-sys/prebuilt/README.md` for how the archive is produced.

#![allow(dead_code)] // `build.rs` and the tests each use a subset.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Bumped when the metadata format or the hash input changes; a prebuilt
/// archive with another format is ignored (and the sources are compiled).
pub const FORMAT: &str = "1";

/// Metadata file next to the prebuilt library.
pub const METADATA_FILE: &str = "metadata.txt";

/// Where release archives put the prebuilt library, relative to the workspace
/// root (`<ZANT_HOME>/zant-ui/third_party/zant-native-prebuilt` once installed).
pub const PREBUILT_DIR: &str = "third_party/zant-native-prebuilt";

/// Points `build.rs` at a prebuilt directory (or directly at the library file).
pub const ENV_PREBUILT: &str = "ZANT_NATIVE_PREBUILT";
/// Makes `build.rs` additionally write the prebuilt archive into this directory.
pub const ENV_OUT: &str = "ZANT_NATIVE_PREBUILT_OUT";
/// Recorded in the metadata by `ENV_OUT` builds; the release scripts set them.
pub const ENV_COMMIT: &str = "ZANT_NATIVE_PREBUILT_COMMIT";
pub const ENV_TOOLCHAIN: &str = "ZANT_NATIVE_PREBUILT_TOOLCHAIN";

pub const SOURCES_MACOS: &str = "crates/zant-sys/prebuilt/sources-macos.txt";
pub const SOURCES_WINDOWS: &str = "crates/zant-sys/prebuilt/sources-windows.txt";

/// The source list for `target_os` (same values as `CARGO_CFG_TARGET_OS`).
pub fn sources_file(target_os: &str) -> Result<&'static str, String> {
    match target_os {
        "macos" => Ok(SOURCES_MACOS),
        "windows" => Ok(SOURCES_WINDOWS),
        other => Err(format!("no native source list for target OS {other}")),
    }
}

/// The prebuilt library's file name for `target_os`.
pub fn library_name(target_os: &str) -> Result<&'static str, String> {
    match target_os {
        "macos" => Ok("libzant_native.a"),
        "windows" => Ok("zant_native.lib"),
        other => Err(format!(
            "no prebuilt native library name for target OS {other}"
        )),
    }
}

/// Reads the canonical source list. Blank lines and `#` comments are ignored.
pub fn read_sources(root: &Path, target_os: &str) -> Result<Vec<String>, String> {
    let relative = sources_file(target_os)?;
    let path = root.join(relative);
    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "cannot read the native source list {}: {error}",
            path.display()
        )
    })?;
    let mut sources = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        sources.push(line.to_owned());
    }
    if sources.is_empty() {
        return Err(format!(
            "the native source list {} is empty",
            path.display()
        ));
    }
    Ok(sources)
}

// ---------------------------------------------------------------------------
// SHA-256 (no build dependencies; `[build-dependencies]` only has `cc`)
// ---------------------------------------------------------------------------

const SHA256_INITIAL: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Incremental SHA-256, the algorithm `shasum -a 256` / `Get-FileHash` use.
pub struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffered: usize,
    length: u64,
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: SHA256_INITIAL,
            buffer: [0; 64],
            buffered: 0,
            length: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.length = self.length.wrapping_add(data.len() as u64);
        while !data.is_empty() {
            let take = (64 - self.buffered).min(data.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&data[..take]);
            self.buffered += take;
            data = &data[take..];
            if self.buffered == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffered = 0;
            }
        }
    }

    pub fn finish(mut self) -> [u8; 32] {
        let bit_length = self.length.wrapping_mul(8);
        self.update(&[0x80]);
        while self.buffered != 56 {
            self.update(&[0]);
        }
        self.update(&bit_length.to_be_bytes());
        debug_assert_eq!(self.buffered, 0);
        let mut digest = [0u8; 32];
        for (chunk, word) in digest.chunks_exact_mut(4).zip(self.state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        digest
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (i, chunk) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes(chunk.try_into().expect("4-byte chunk"));
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

/// Lower-case hex of the SHA-256 of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    to_hex(&hasher.finish())
}

fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

// ---------------------------------------------------------------------------
// Source hash
// ---------------------------------------------------------------------------

/// SHA-256 over the canonical source list, byte for byte:
///
/// ```text
/// for each path in list order:
///     <path as written in the list> + "\n"
///     <contents of that file>
///     "\n"
/// ```
///
/// A shell equivalent (used by `scripts/release/prebuilt-check.sh` to verify
/// this implementation independently):
///
/// ```sh
/// { while IFS= read -r f; do case "$f" in ''|'#'*) continue;; esac
///     printf '%s\n' "$f"; cat -- "$f"; printf '\n'; done < sources-macos.txt
/// } | shasum -a 256
/// ```
pub fn source_hash(root: &Path, sources: &[String]) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    for relative in sources {
        let path = root.join(relative);
        hasher.update(relative.as_bytes());
        hasher.update(b"\n");
        let mut file = fs::File::open(&path).map_err(|error| {
            format!("cannot hash the native source {}: {error}", path.display())
        })?;
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        hasher.update(b"\n");
    }
    Ok(to_hex(&hasher.finish()))
}

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

/// `metadata.txt` next to the prebuilt library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub format: String,
    pub platform: String,
    pub source_hash: String,
    pub source_commit: String,
    pub toolchain: String,
    pub built_at: String,
}

impl Metadata {
    /// Parses the `key=value` format; unknown keys are kept out and ignored so
    /// the format can grow without invalidating old archives.
    pub fn parse(text: &str) -> Result<Metadata, String> {
        let mut fields = BTreeMap::new();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!(
                    "metadata line {} is not key=value: {line}",
                    number + 1
                ));
            };
            fields.insert(key.trim().to_owned(), value.trim().to_owned());
        }
        let required = |key: &str| -> Result<String, String> {
            fields
                .get(key)
                .cloned()
                .ok_or_else(|| format!("metadata has no `{key}=` line"))
        };
        Ok(Metadata {
            format: required("format")?,
            platform: required("platform")?,
            source_hash: required("source_hash")?,
            source_commit: fields.get("source_commit").cloned().unwrap_or_default(),
            toolchain: fields.get("toolchain").cloned().unwrap_or_default(),
            built_at: fields.get("built_at").cloned().unwrap_or_default(),
        })
    }

    pub fn encode(&self) -> String {
        format!(
            "# Built by crates/zant-sys/build.rs; see crates/zant-sys/prebuilt/README.md.\n\
             format={}\n\
             platform={}\n\
             source_hash={}\n\
             source_commit={}\n\
             toolchain={}\n\
             built_at={}\n",
            self.format,
            self.platform,
            self.source_hash,
            self.source_commit,
            self.toolchain,
            self.built_at,
        )
    }

    pub fn read(path: &Path) -> Result<Metadata, String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        Metadata::parse(&text)
    }
}

/// `metadata.txt` inside a prebuilt directory.
pub fn metadata_path(dir: &Path) -> PathBuf {
    dir.join(METADATA_FILE)
}

/// Writes `metadata.txt` for a freshly built archive. `ENV_COMMIT` /
/// `ENV_TOOLCHAIN` are recorded as given (the release scripts set them);
/// otherwise the values are `unknown`.
pub fn write_metadata(out_dir: &Path, target_os: &str, root: &Path) -> Result<Metadata, String> {
    let sources = read_sources(root, target_os)?;
    let metadata = Metadata {
        format: FORMAT.to_owned(),
        platform: target_os.to_owned(),
        source_hash: source_hash(root, &sources)?,
        source_commit: env_value(ENV_COMMIT),
        toolchain: env_value(ENV_TOOLCHAIN),
        built_at: unix_seconds().to_string(),
    };
    fs::create_dir_all(out_dir)
        .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;
    let path = metadata_path(out_dir);
    fs::write(&path, metadata.encode())
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(metadata)
}

fn env_value(key: &str) -> String {
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => value.trim().to_owned(),
        _ => "unknown".to_owned(),
    }
}

/// Seconds since the Unix epoch, for `built_at`.
pub fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Where a candidate prebuilt library came from (for the build warning).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// `ZANT_NATIVE_PREBUILT` is set.
    Env,
    /// `<root>/third_party/zant-native-prebuilt`, as shipped in releases.
    Default,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Env => "ZANT_NATIVE_PREBUILT",
            Source::Default => PREBUILT_DIR,
        }
    }
}

/// A prebuilt library that may or may not be valid; `verify` decides.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub source: Source,
    /// Directory that should contain `metadata.txt`.
    pub dir: PathBuf,
    /// The static library itself (`libzant_native.a` / `zant_native.lib`).
    pub library: PathBuf,
}

/// The prebuilt archive location `build.rs` looks at. `ENV_PREBUILT` wins and
/// may name the directory or the library file itself.
pub fn candidate(root: &Path, target_os: &str) -> Result<Candidate, String> {
    let library_name = library_name(target_os)?;
    if let Ok(value) = env::var(ENV_PREBUILT) {
        let value = value.trim();
        if !value.is_empty() {
            let path = PathBuf::from(value);
            let looks_like_library = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == library_name || name == "libzant_native.a");
            let (dir, library) = if path.is_dir() || (!path.exists() && !looks_like_library) {
                (path.clone(), path.join(library_name))
            } else {
                let dir = path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."));
                (dir, path)
            };
            return Ok(Candidate {
                source: Source::Env,
                dir,
                library,
            });
        }
    }
    let dir = root.join(PREBUILT_DIR);
    Ok(Candidate {
        source: Source::Default,
        dir: dir.clone(),
        library: dir.join(library_name),
    })
}

/// A verified prebuilt library: the metadata matches this tree's native
/// sources, so linking it is equivalent to compiling them.
#[derive(Clone, Debug)]
pub struct Prebuilt {
    pub dir: PathBuf,
    pub library: PathBuf,
    pub metadata: Metadata,
}

/// Checks that `candidate` exists and matches the current native sources.
/// The error text is what `build.rs` puts in its fallback warning.
pub fn verify(candidate: &Candidate, root: &Path, target_os: &str) -> Result<Prebuilt, String> {
    if !candidate.library.is_file() {
        return Err(format!(
            "{} from {} does not exist",
            candidate.library.display(),
            candidate.source.label()
        ));
    }
    let metadata_path = metadata_path(&candidate.dir);
    let metadata = Metadata::read(&metadata_path)?;
    if metadata.format != FORMAT {
        return Err(format!(
            "{} has format {} but this build.rs needs format {FORMAT}",
            metadata_path.display(),
            metadata.format
        ));
    }
    if metadata.platform != target_os {
        return Err(format!(
            "{} was built for {} but this build targets {target_os}",
            candidate.library.display(),
            metadata.platform
        ));
    }
    let sources = read_sources(root, target_os)?;
    let expected = source_hash(root, &sources)?;
    if metadata.source_hash != expected {
        return Err(format!(
            "the native sources changed (metadata source_hash {} commit {}, the tree now hashes \
             to {expected})",
            metadata.source_hash, metadata.source_commit
        ));
    }
    Ok(Prebuilt {
        dir: candidate.dir.clone(),
        library: candidate.library.clone(),
        metadata,
    })
}
