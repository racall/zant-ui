#!/bin/bash
# Installs the zant CLI and Zant UI on macOS (Apple silicon), ready to use:
#
#   curl -fsSL https://raw.githubusercontent.com/racall/zant-ui/main/install.sh | bash
#
# 1. Zig: uses `zig` from PATH when it is the version Zant UI needs,
#    otherwise downloads the official build into $ZANT_HOME/zig.
# 2. Checks the Mac (Apple silicon, macOS 13+, Xcode command line tools,
#    which apps need to link against the macOS SDK).
# 3. Downloads the release archive (zant, Zant UI and the prebuilt Skia;
#    nothing is compiled) into $ZANT_HOME and links `zant` into
#    $ZANT_BIN_DIR, adding that to PATH.
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

ZIG_VERSION="0.17.1"
ZIG_URL="https://ziglang.org/download/${ZIG_VERSION}/zig-aarch64-macos-${ZIG_VERSION}.tar.xz"
ZIG_SHA256="75f1bc7a1e9852f17ec9a0c1895f767c4ec56d46bc8a834920eece1d0359dd8d"

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

# ---- 1. Zig
ZIG=""
if command -v zig >/dev/null 2>&1 && [ "$(zig version 2>/dev/null)" = "$ZIG_VERSION" ]; then
  ZIG="$(command -v zig)"
  ok "Zig ${ZIG_VERSION}：${ZIG}"
elif [ -x "$ZANT_HOME/zig/$ZIG_VERSION/zig" ]; then
  ZIG="$ZANT_HOME/zig/$ZIG_VERSION/zig"
  ok "Zig ${ZIG_VERSION}：${ZIG}"
elif command -v zig >/dev/null 2>&1; then
  step "已安装的 Zig 是 $(zig version 2>/dev/null || echo 未知版本)，Zant UI 需要 ${ZIG_VERSION}：另外下载一份给 zant 用（不影响已有的）"
else
  step "没有找到 Zig：下载官方的 Zig ${ZIG_VERSION}"
fi

# ---- 2. the Mac
[ "$(uname -s)" = "Darwin" ] || die "install.sh 用于 macOS；Windows 用 install.ps1（见 Zant UI 的 README「安装」）"
[ "$(uname -m)" = "arm64" ] || die "Zant UI 在 macOS 上需要 Apple 芯片（预编译的 Skia 只有 arm64 版本）"
major="$(sw_vers -productVersion | cut -d. -f1)"
[ "$major" -ge 13 ] || die "需要 macOS 13 或更新版本（现在是 $(sw_vers -productVersion)）"
if ! xcode-select -p >/dev/null 2>&1; then
  xcode-select --install >/dev/null 2>&1 || true
  die "需要 Xcode 命令行工具（编译应用要用 macOS SDK）：已打开安装窗口，装好后重新运行这条命令"
fi

if [ -z "$ZIG" ]; then
  curl -fL --retry 3 --connect-timeout 20 --progress-bar "$ZIG_URL" -o "$tmp/zig.tar.xz" || die "下载 Zig 失败：$ZIG_URL"
  actual="$(shasum -a 256 "$tmp/zig.tar.xz" | awk '{print $1}')"
  [ "$actual" = "$ZIG_SHA256" ] || die "Zig 下载的文件校验不对（$actual），请重试"
  tar -xJf "$tmp/zig.tar.xz" -C "$tmp"
  mkdir -p "$ZANT_HOME/zig"
  rm -rf "$ZANT_HOME/zig/$ZIG_VERSION"
  mv "$tmp/zig-aarch64-macos-${ZIG_VERSION}" "$ZANT_HOME/zig/$ZIG_VERSION"
  ZIG="$ZANT_HOME/zig/$ZIG_VERSION/zig"
  ok "Zig ${ZIG_VERSION}：${ZIG}"
fi

# ---- 3. zant and Zant UI
latest="$(curl -fsSL --retry 3 --connect-timeout 20 --max-time 60 "$DOWNLOAD/version.txt" 2>/dev/null | tr -d '[:space:]')" ||
  die "无法获取版本信息：$DOWNLOAD/version.txt"
[ -n "$latest" ] || die "无法获取版本信息：$DOWNLOAD/version.txt"
installed="$(cat "$ZANT_HOME/zant-ui/.zant-version" 2>/dev/null | tr -d '[:space:]' || true)"
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
  [ -x "$tmp/new/bin/zant" ] && [ -f "$tmp/new/zant-ui/build.zig.zon" ] || die "发布包内容不完整"

  # swap in the new version (projects refer to $ZANT_HOME/zant-ui, so the path stays)
  mkdir -p "$ZANT_HOME/bin"
  rm -rf "$ZANT_HOME/zant-ui.old"
  [ -d "$ZANT_HOME/zant-ui" ] && mv "$ZANT_HOME/zant-ui" "$ZANT_HOME/zant-ui.old"
  mv "$tmp/new/zant-ui" "$ZANT_HOME/zant-ui"
  mv -f "$tmp/new/bin/zant" "$ZANT_HOME/bin/zant"
  rm -rf "$ZANT_HOME/zant-ui.old"
  ok "zant ${latest}：$ZANT_HOME"
fi

# ---- 4. PATH
mkdir -p "$ZANT_BIN_DIR"
link="$ZANT_BIN_DIR/zant"
if [ -e "$link" ] && [ ! -L "$link" ]; then
  die "$link 已存在且不是链接：移走后再运行"
fi
ln -sfn "$ZANT_HOME/bin/zant" "$link"
# Zig on PATH too when there is none (for `zig build test` in projects)
if ! command -v zig >/dev/null 2>&1; then
  ln -sfn "$ZIG" "$ZANT_BIN_DIR/zig"
fi
case ":$PATH:" in
*":$ZANT_BIN_DIR:"*) ;;
*)
  rc="$HOME/.zshrc"
  case "${SHELL:-}" in */bash) rc="$HOME/.bash_profile" ;; esac
  line="export PATH=\"$ZANT_BIN_DIR:\$PATH\""
  if [ "${ZANT_NO_MODIFY_PATH:-0}" = 1 ]; then
    step "请把 $ZANT_BIN_DIR 加到 PATH：$line"
  elif ! grep -qsF "$line" "$rc"; then
    printf '\n# zant (Zant UI)\n%s\n' "$line" >>"$rc"
    ok "已把 $ZANT_BIN_DIR 加到 PATH（$rc），新开的终端里生效"
  fi
  ;;
esac

printf '\n\033[32m完成\033[0m。试试：\n\n'
printf '  zant create my-app\n  cd my-app && zant run\n\n'
printf '以后更新：zant update\n\n'
