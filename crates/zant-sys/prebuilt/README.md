# Prebuilt native library (WP-6.6)

`crates/zant-sys/build.rs` normally compiles Zant UI's native layer — the Skia
bridge (`src/renderer/skia/*.cpp`, `skia_bridge_mac.mm` / `skia_bridge_win.cpp`),
the AppKit / Win32 window backend and the PDF backend — on every clean app
build. That is the slow part of a first build (tens of seconds to a few
minutes), and on Windows it needs Visual Studio's C++ tools.

WP-6.6 removes that cost from installed apps: the release workflow builds the
native objects once per platform, links them into one static archive and ships
it inside `zant-ui/third_party/zant-native-prebuilt/`. When `build.rs` finds
that archive it links it instead of compiling; Skia itself was already
prebuilt, so nothing gets compiled in C++ at all.

The design mirrors the Zig `third_party/zant-skia-prebuilt` +
`hasPrebuiltBridge()` mechanism (`build.zig`), with one difference the Rust
port needs: the archive only contains the objects build.rs would compile, so
the build must prove it matches the tree — not just check that a file exists.

## Contents

| File | Purpose |
|---|---|
| `libzant_native.a` (macOS) / `zant_native.lib` (Windows) | The native objects, one archive (`ar` / `lib.exe`). |
| `metadata.txt` | SHA-256 of the sources the archive was built from, the commit, the toolchain and the build time. |

`metadata.txt` is `key=value`, no dependencies (build.rs has no serde):

```text
# Built by crates/zant-sys/build.rs (WP-6.6); see crates/zant-sys/prebuilt/README.md.
format=1
platform=macos
source_hash=6c4c…  # see below
source_commit=3cfff3d…
toolchain=Apple clang version 21.0.0 (clang-2100.4.1.1)
built_at=1762000000
```

`source_hash` is the SHA-256 over the canonical source list
(`sources-macos.txt` / `sources-windows.txt`) in list order, byte for byte:

```text
for each path in list order:
    <path as written in the list> + "\n"
    <contents of that file>
    "\n"
```

Headers are part of the list, so a header edit invalidates the archive too.
A shell equivalent (this is how `scripts/release/prebuilt-check.sh` verifies
the Rust implementation independently):

```sh
{ while IFS= read -r f; do case "$f" in ''|'#'*) continue;; esac
    printf '%s\n' "$f"; cat -- "$f"; printf '\n'; done < crates/zant-sys/prebuilt/sources-macos.txt
} | shasum -a 256
```

## How a build uses it

1. `ZANT_NATIVE_PREBUILT` names the directory (or the archive file) when set;
   otherwise `<workspace root>/third_party/zant-native-prebuilt/` is used, which
   is where release archives put it (`<ZANT_HOME>/zant-ui/third_party/...`).
2. `build.rs` recomputes `source_hash` from the tree and compares it with
   `metadata.txt` (and checks `format` / `platform`).
3. Match → it prints `zant-sys: using the prebuilt native library …`, adds
   `-l zant_native` and links Skia + the system libraries as usual.
4. No match / no metadata / wrong platform → it prints
   `zant-sys: ignoring the prebuilt native library: <reason>; compiling the
   native sources instead` and does the normal compile. A stale archive can
   never silently change behaviour.
5. With no archive present, the build behaves exactly like before WP-6.6.

`ZANT_NATIVE_PREBUILT_OUT=<dir>` makes `build.rs` run the normal compile *and*
write `<dir>/libzant_native.a` (plus `zant_native.lib` on MSVC) and
`<dir>/metadata.txt`. `ZANT_NATIVE_PREBUILT_COMMIT` /
`ZANT_NATIVE_PREBUILT_TOOLCHAIN` are recorded in the metadata (the release
scripts set them).

## Building the archive

```sh
# macOS (also runs from CI's release workflow)
./scripts/release/prebuilt.sh                 # -> target/prebuilt/macos/
# Windows
powershell -File scripts/release/prebuilt.ps1 # -> target\prebuilt\windows\
```

`scripts/release/package.{sh,ps1}` copy the directory into
`zant-ui/third_party/zant-native-prebuilt/` and fail when it is missing (pass
`ZANT_ALLOW_NO_PREBUILT=1` / `-AllowNoPrebuilt` for a local package without
one). `scripts/release/prebuilt-check.sh` is the end-to-end test: it builds the
archive, links `zant-demo` with it, then checks that a tampered archive falls
back to compiling and that the cold path still works.

## Notes

- The archive is not checked into git and a source checkout never has
  `third_party/zant-native-prebuilt/`; `third_party/` itself is gitignored.
- C++ ABI: the archive is linked into the same executable as the Rust code, so
  on Windows it must be built with MSVC's STL and `/MD` like the Skia archives
  it links against (`build_bridge.ps1` uses the same flags).
- macOS: the manifest/flags match `build.zig`'s native compile; the archive is
  built with the same source list, so a prebuilt and a from-source build are
  the same program.
