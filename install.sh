#!/bin/bash
# Installs Zant UI and its `zant` CLI on macOS (Apple silicon):
#
#   curl -fsSL https://raw.githubusercontent.com/racall/zant-ui/main/install.sh | bash
#
# 1. Zig: uses `zig` from PATH when it is the version Zant UI needs,
#    otherwise downloads the official build into $ZANT_HOME/zig.
# 2. Checks the Mac (Apple silicon, macOS 13+, Xcode command line tools).
# 3. Clones Zant UI into $ZANT_HOME/zant-ui (or updates it).
# 4. Downloads the pinned Skia (scripts/fetch_skia.sh).
# 5. Builds `zant` and links it into $ZANT_BIN_DIR, adding that to PATH.
#
# Running it again updates everything (`zant update` does the same for the
# code). Settings (environment variables):
#   ZANT_HOME     install folder (default ~/.zant)
#   ZANT_BIN_DIR  where the `zant` link goes (default ~/.local/bin)
#   ZANT_REPO     git URL (default https://github.com/racall/zant-ui.git)
#   ZANT_REF      branch or tag (default main)
#   ZANT_NO_MODIFY_PATH=1  do not touch shell startup files
set -euo pipefail

ZIG_VERSION="0.17.1"
ZIG_URL="https://ziglang.org/download/${ZIG_VERSION}/zig-aarch64-macos-${ZIG_VERSION}.tar.xz"
ZIG_SHA256="75f1bc7a1e9852f17ec9a0c1895f767c4ec56d46bc8a834920eece1d0359dd8d"

ZANT_HOME="${ZANT_HOME:-$HOME/.zant}"
ZANT_BIN_DIR="${ZANT_BIN_DIR:-$HOME/.local/bin}"
ZANT_REPO="${ZANT_REPO:-https://github.com/racall/zant-ui.git}"
ZANT_REF="${ZANT_REF:-main}"
SRC="$ZANT_HOME/zant-ui"

step() { printf '  \033[36m→\033[0m %s\n' "$*"; }
ok() { printf '  \033[32m✓\033[0m %s\n' "$*"; }
die() {
  printf '\n  \033[31m✗\033[0m %s\n\n' "$*" >&2
  exit 1
}

printf '\n\033[36mZant UI 安装\033[0m（%s）\n' "$ZANT_HOME"

# ---- 1. Zig
ZIG=""
if command -v zig >/dev/null 2>&1 && [ "$(zig version 2>/dev/null)" = "$ZIG_VERSION" ]; then
  ZIG="$(command -v zig)"
  ok "Zig ${ZIG_VERSION}：${ZIG}"
elif [ -x "$ZANT_HOME/zig/$ZIG_VERSION/zig" ]; then
  ZIG="$ZANT_HOME/zig/$ZIG_VERSION/zig"
  ok "Zig ${ZIG_VERSION}：${ZIG}"
else
  if command -v zig >/dev/null 2>&1; then
    step "已安装的 Zig 是 $(zig version 2>/dev/null || echo 未知版本)，Zant UI 需要 ${ZIG_VERSION}：另外下载一份给 zant 用（不影响已有的）"
  else
    step "没有找到 Zig：下载官方的 Zig ${ZIG_VERSION}"
  fi
fi

# ---- 2. the Mac
[ "$(uname -s)" = "Darwin" ] || die "install.sh 用于 macOS；Windows 请用 install.ps1（见 README「安装」）"
[ "$(uname -m)" = "arm64" ] || die "Zant UI 在 macOS 上需要 Apple 芯片（预编译的 Skia 只有 arm64 版本）"
major="$(sw_vers -productVersion | cut -d. -f1)"
[ "$major" -ge 13 ] || die "需要 macOS 13 或更新版本（现在是 $(sw_vers -productVersion)）"
if ! xcode-select -p >/dev/null 2>&1; then
  xcode-select --install >/dev/null 2>&1 || true
  die "需要 Xcode 命令行工具：已打开安装窗口，装好后重新运行这条安装命令"
fi
command -v git >/dev/null 2>&1 || die "找不到 git（Xcode 命令行工具自带）"

if [ -z "$ZIG" ]; then
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  curl -fL --retry 3 --progress-bar "$ZIG_URL" -o "$tmp/zig.tar.xz" || die "下载 Zig 失败：$ZIG_URL"
  actual="$(shasum -a 256 "$tmp/zig.tar.xz" | awk '{print $1}')"
  [ "$actual" = "$ZIG_SHA256" ] || die "Zig 下载的文件校验不对（$actual），请重试"
  mkdir -p "$ZANT_HOME/zig"
  rm -rf "$ZANT_HOME/zig/$ZIG_VERSION"
  tar -xJf "$tmp/zig.tar.xz" -C "$tmp"
  mv "$tmp/zig-aarch64-macos-${ZIG_VERSION}" "$ZANT_HOME/zig/$ZIG_VERSION"
  ZIG="$ZANT_HOME/zig/$ZIG_VERSION/zig"
  ok "Zig ${ZIG_VERSION}：${ZIG}"
fi

# ---- 3. the source
if [ -d "$SRC/.git" ]; then
  step "更新 Zant UI（${ZANT_REF}）"
  git -C "$SRC" fetch --quiet origin "$ZANT_REF"
  git -C "$SRC" checkout --quiet "$ZANT_REF"
  git -C "$SRC" merge --quiet --ff-only "origin/$ZANT_REF" 2>/dev/null ||
    git -C "$SRC" merge --quiet --ff-only FETCH_HEAD ||
    die "$SRC 有本地修改，不能自动更新：处理后再运行"
else
  [ -e "$SRC" ] && die "$SRC 已存在但不是 git 仓库：移走后再运行"
  step "下载 Zant UI（${ZANT_REF}）"
  mkdir -p "$ZANT_HOME"
  git clone --quiet --branch "$ZANT_REF" "$ZANT_REPO" "$SRC"
fi
ok "Zant UI：$SRC（$(git -C "$SRC" rev-parse --short HEAD)）"

# ---- 4. Skia
step "下载 Skia（只在缺少或版本变化时下载）"
bash "$SRC/scripts/fetch_skia.sh"

# ---- 5. zant
step "编译 zant（第一次要几分钟）"
(cd "$SRC" && "$ZIG" build cli) || die "编译 zant 失败（见上面的错误）"
mkdir -p "$ZANT_BIN_DIR"
link="$ZANT_BIN_DIR/zant"
if [ -e "$link" ] && [ ! -L "$link" ]; then
  die "$link 已存在且不是链接：移走后再运行"
fi
ln -sfn "$SRC/zig-out/bin/zant" "$link"
ok "zant：$link"

# Zig on PATH too, when there is none (for `zig build test` in projects)
if ! command -v zig >/dev/null 2>&1; then
  ln -sfn "$ZIG" "$ZANT_BIN_DIR/zig"
fi

case ":$PATH:" in
*":$ZANT_BIN_DIR:"*) on_path=1 ;;
*) on_path=0 ;;
esac
if [ "$on_path" = 0 ]; then
  rc="$HOME/.zshrc"
  case "${SHELL:-}" in */bash) rc="$HOME/.bash_profile" ;; esac
  line="export PATH=\"$ZANT_BIN_DIR:\$PATH\""
  if [ "${ZANT_NO_MODIFY_PATH:-0}" = 1 ]; then
    step "请把 $ZANT_BIN_DIR 加到 PATH：$line"
  elif ! grep -qsF "$line" "$rc"; then
    printf '\n# Zant UI\n%s\n' "$line" >>"$rc"
    ok "已把 $ZANT_BIN_DIR 加到 PATH（$rc），新开的终端里生效"
  fi
fi

printf '\n\033[32m安装完成\033[0m。试试：\n\n'
printf '  zant create my-app\n  cd my-app && zant run\n\n'
printf '以后更新：zant update\n\n'
