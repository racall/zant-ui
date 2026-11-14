#!/bin/bash
# Installs the zant CLI and Zant UI on macOS (Apple silicon), ready to use:
#
#   curl -fsSL https://raw.githubusercontent.com/racall/zant-ui/main/install.sh | bash
#
# 1. Rust: installs rustup into ~/.cargo when it is missing and makes sure the
#    Rust toolchain Zant UI pins (zant-ui/rust-toolchain.toml) is installed.
#    No Zig download any more; `cargo` builds the apps.
# 2. Checks the Mac (Apple silicon, macOS 13+, Xcode command line tools,
#    which apps need to link against the macOS SDK).
# 3. Downloads the release archive (zant, the Rust workspace with the native
#    sources and the prebuilt Skia; nothing is compiled) into $ZANT_HOME and
#    links `zant` into $ZANT_BIN_DIR, adding that to PATH.
#
# Running it again (or `zant update`) updates to the latest release.
# Settings (environment variables):
#   ZANT_HOME      install folder (default ~/.zant)
#   ZANT_BIN_DIR   where the `zant` link goes (default ~/.local/bin)
#   ZANT_VERSION   a release tag, e.g. v0.1.0 (default: the latest)
#   ZANT_DOWNLOAD  where the release files are (default: GitHub Releases)
#   ZANT_FORCE=1   download again even when up to date
#   ZANT_NO_MODIFY_PATH=1  do not touch shell startup files
set -euo pipefail

# rustup-init for Apple silicon; the .sha256 next to it is published by
# static.rust-lang.org. The version of rustup itself is not pinned (Rust's
# download server has no per-version paths); the *toolchain* below is.
RUSTUP_TARGET="aarch64-apple-darwin"
RUSTUP_URL="https://static.rust-lang.org/rustup/dist/${RUSTUP_TARGET}/rustup-init"
# fallback when zant-ui/rust-toolchain.toml cannot be read yet
FALLBACK_TOOLCHAIN="stable"
# the components rust-toolchain.toml asks for (rustup does not install them
# with --profile minimal)
TOOLCHAIN_COMPONENTS="rustfmt clippy"

ZANT_HOME="${ZANT_HOME:-$HOME/.zant}"
ZANT_BIN_DIR="${ZANT_BIN_DIR:-$HOME/.local/bin}"
if [ -n "${ZANT_VERSION:-}" ]; then
  DEFAULT_DOWNLOAD="https://github.com/racall/zant-ui/releases/download/$ZANT_VERSION"
else
  DEFAULT_DOWNLOAD="https://github.com/racall/zant-ui/releases/latest/download"
fi
DOWNLOAD="${ZANT_DOWNLOAD:-$DEFAULT_DOWNLOAD}"
ARCHIVE="zant-macos-arm64.tar.gz"

step() { printf '  \033[36m→\033[0m %s\n' "$*"; }
ok() { printf '  \033[32m✓\033[0m %s\n' "$*"; }
die() {
  printf '\n  \033[31m✗\033[0m %s\n\n' "$*" >&2
  exit 1
}
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

printf '\n\033[36m安装 zant\033[0m（%s）\n' "$ZANT_HOME"

# ---- 1. the Mac
[ "$(uname -s)" = "Darwin" ] || die "install.sh 用于 macOS；Windows 用 install.ps1（见 Zant UI 的 README「安装」）"
[ "$(uname -m)" = "arm64" ] || die "Zant UI 在 macOS 上需要 Apple 芯片（预编译的 Skia 只有 arm64 版本）"
major="$(sw_vers -productVersion | cut -d. -f1)"
[ "$major" -ge 13 ] || die "需要 macOS 13 或更新版本（现在是 $(sw_vers -productVersion)）"
if ! xcode-select -p >/dev/null 2>&1; then
  xcode-select --install >/dev/null 2>&1 || true
  die "需要 Xcode 命令行工具（编译应用要用 macOS SDK）：已打开安装窗口，装好后重新运行这条命令"
fi

# ---- 2. Rust: rustup (installed here when missing) + the pinned toolchain
# `zant update` can run this script from a shell where ~/.cargo/bin is not on
# PATH yet, so look there too.
if [ -x "$HOME/.cargo/bin/rustup" ]; then
  PATH="$HOME/.cargo/bin:$PATH"
fi
if command -v rustup >/dev/null 2>&1; then
  ok "rustup：$(command -v rustup)"
else
  step "没有找到 rustup：下载 Rust 安装器（rustup-init）"
  curl -fL --retry 3 --connect-timeout 20 --progress-bar "$RUSTUP_URL" -o "$tmp/rustup-init" ||
    die "下载 Rust 安装器失败：$RUSTUP_URL"
  expected="$(curl -fsSL --retry 3 --connect-timeout 20 --max-time 60 "$RUSTUP_URL.sha256" | awk '{print $1}')" ||
    die "下载 Rust 安装器的校验值失败"
  actual="$(shasum -a 256 "$tmp/rustup-init" | awk '{print $1}')"
  [ "$actual" = "$expected" ] || die "Rust 安装器校验不对（$actual），请重试"
  chmod +x "$tmp/rustup-init"
  # -y: no questions (this script itself is piped from curl); --default-toolchain
  # none: the pinned channel is installed below, once the release is unpacked.
  rustup_args=(-y --profile minimal --default-toolchain none)
  if [ "${ZANT_NO_MODIFY_PATH:-0}" = 1 ]; then
    rustup_args+=(--no-modify-path)
  fi
  "$tmp/rustup-init" "${rustup_args[@]}" ||
    die "安装 rustup 失败（见上面的错误）"
  PATH="$HOME/.cargo/bin:$PATH"
  command -v rustup >/dev/null 2>&1 ||
    die "安装后仍找不到 rustup：重新打开终端后再运行这条命令"
  ok "rustup：$(command -v rustup)"
fi

# ---- 3. zant and Zant UI
latest="$(curl -fsSL --retry 3 --connect-timeout 20 --max-time 60 "$DOWNLOAD/version.txt" 2>/dev/null | tr -d '[:space:]')" ||
  die "无法获取版本信息：$DOWNLOAD/version.txt"
[ -n "$latest" ] || die "无法获取版本信息：$DOWNLOAD/version.txt"
installed="$(cat "$ZANT_HOME/zant-ui/.zant-version" 2>/dev/null | tr -d '[:space:]' || true)"
new_ui=""
if [ "$installed" = "$latest" ] && [ -x "$ZANT_HOME/bin/zant" ] && [ "${ZANT_FORCE:-0}" != 1 ]; then
  ok "zant ${latest} 已经是最新"
else
  if [ -n "$installed" ]; then step "更新 zant：${installed} → ${latest}"; else step "下载 zant ${latest}"; fi
  curl -fL --retry 3 --connect-timeout 20 --progress-bar "$DOWNLOAD/$ARCHIVE" -o "$tmp/$ARCHIVE" || die "下载失败：$DOWNLOAD/$ARCHIVE"
  expected="$(curl -fsSL --retry 3 --connect-timeout 20 --max-time 60 "$DOWNLOAD/$ARCHIVE.sha256" | tr -d '[:space:]')" || die "下载校验值失败"
  actual="$(shasum -a 256 "$tmp/$ARCHIVE" | awk '{print $1}')"
  [ "$actual" = "$expected" ] || die "下载的文件校验不对，请重试"
  mkdir -p "$tmp/new"
  tar -xzf "$tmp/$ARCHIVE" -C "$tmp/new"
  new_ui="$tmp/new/zant-ui"
  [ -x "$tmp/new/bin/zant" ] && [ -f "$new_ui/Cargo.toml" ] || die "发布包内容不完整"
fi

# ---- 4. the Rust toolchain Zant UI pins (zant-ui/rust-toolchain.toml)
pin_dir="${new_ui:-$ZANT_HOME/zant-ui}"
channel=""
if [ -f "$pin_dir/rust-toolchain.toml" ]; then
  channel="$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$pin_dir/rust-toolchain.toml" | head -1)"
fi
channel="${channel:-$FALLBACK_TOOLCHAIN}"
component_args=()
for component in $TOOLCHAIN_COMPONENTS; do
  component_args+=(--component "$component")
done
step "确保 Rust 工具链 ${channel}（编译应用用；第一次要下载几百 MB）"
rustup toolchain install "$channel" --profile minimal "${component_args[@]}" ||
  die "安装 Rust 工具链失败：rustup toolchain install ${channel}"
ok "Rust 工具链：$channel"
# a fresh rustup has no default toolchain: apps run cargo outside zant-ui too
if ! rustup toolchain list 2>/dev/null | grep -q '(default)'; then
  rustup default "$channel" >/dev/null 2>&1 || true
fi
# run it outside the caller's directory: a rust-toolchain.toml there would
# make the rustup shim pick (and try to install) that project's toolchain
cargo_version="$(cd "$tmp" && cargo --version 2>/dev/null || true)"
[ -n "$cargo_version" ] ||
  die "找不到 cargo：确认 $HOME/.cargo/bin 在 PATH 上（重新打开终端后再试）"
ok "$cargo_version"

# swap in the new version (projects refer to $ZANT_HOME/zant-ui, so the path stays)
if [ -n "$new_ui" ]; then
  mkdir -p "$ZANT_HOME/bin"
  rm -rf "$ZANT_HOME/zant-ui.old"
  [ -d "$ZANT_HOME/zant-ui" ] && mv "$ZANT_HOME/zant-ui" "$ZANT_HOME/zant-ui.old"
  mv "$new_ui" "$ZANT_HOME/zant-ui"
  mv -f "$tmp/new/bin/zant" "$ZANT_HOME/bin/zant"
  rm -rf "$ZANT_HOME/zant-ui.old"
  ok "zant ${latest}：$ZANT_HOME"
fi

# ---- 5. PATH
mkdir -p "$ZANT_BIN_DIR"
link="$ZANT_BIN_DIR/zant"
if [ -e "$link" ] && [ ! -L "$link" ]; then
  die "$link 已存在且不是链接：移走后再运行"
fi
ln -sfn "$ZANT_HOME/bin/zant" "$link"
case ":$PATH:" in
*":$ZANT_BIN_DIR:"*) ;;
*)
  rc="$HOME/.zshrc"
  case "${SHELL:-}" in */bash) rc="$HOME/.bash_profile" ;; esac
  line="export PATH=\"$ZANT_BIN_DIR:\$PATH\""
  if [ "${ZANT_NO_MODIFY_PATH:-0}" = 1 ]; then
    step "请把 $ZANT_BIN_DIR 加到 PATH：$line"
    step "Rust 也需要 PATH：export PATH=\"\$HOME/.cargo/bin:\$PATH\""
  elif ! grep -qsF "$line" "$rc"; then
    printf '\n# zant (Zant UI)\n%s\n' "$line" >>"$rc"
    ok "已把 $ZANT_BIN_DIR 加到 PATH（$rc），新开的终端里生效"
  fi
  ;;
esac

printf '\n\033[32m完成\033[0m。试试：\n\n'
printf '  zant create my-app\n  cd my-app && zant run\n\n'
printf '以后更新：zant update\n\n'
