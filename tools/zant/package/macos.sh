#!/bin/bash
# macOS release steps run by `zant build` after it has built and signed
# zig-out/<title>.app. zant asks for and resolves the settings
# (build.config.json, the keychain, environment variables) and passes them
# here as ZANT_* environment variables; secrets never appear on a command
# line or in the log.
#
#   macos.sh developer-id     DMG (or zip), signed; optional notarization + staple
#   macos.sh app-store        .pkg signed with the installer certificate; optional upload
#   macos.sh import-cert      CI: import a .p12 into a temporary keychain
#   macos.sh remove-keychain  CI: remove that keychain again
#
# Common:      ZANT_APP, ZANT_TITLE, ZANT_VERSION, ZANT_OUT (output folder),
#              ZANT_IDENTITY ("none" = ad-hoc), ZANT_KEYCHAIN (optional)
# developer-id: ZANT_FORMAT (dmg | zip), ZANT_DMG_BACKGROUND (PNG; empty = plain
#              window), ZANT_NOTARIZE (1 | 0) and one of
#              ZANT_NOTARY_PROFILE | ZANT_NOTARY_KEY + _KEY_ID + _ISSUER |
#              ZANT_NOTARY_APPLE_ID + _PASSWORD + _TEAM
# app-store:   ZANT_INSTALLER_IDENTITY, ZANT_UPLOAD (1 | 0),
#              ZANT_ASC_KEY (.p8 path), ZANT_ASC_KEY_ID, ZANT_ASC_ISSUER
# import-cert: ZANT_CERT (.p12 path), ZANT_CERT_PASSWORD, ZANT_KEYCHAIN
set -euo pipefail

step() { printf '  \033[36m→\033[0m %s\n' "$*" >&2; }
ok() { printf '  \033[32m✓\033[0m %s\n' "$*" >&2; }
die() { printf '\n  \033[31m✗\033[0m %s\n\n' "$*" >&2; exit 1; }

keychain_args=()
[ -n "${ZANT_KEYCHAIN:-}" ] && keychain_args=(--keychain "$ZANT_KEYCHAIN")

notary_auth() {
  if [ -n "${ZANT_NOTARY_PROFILE:-}" ]; then
    auth=(--keychain-profile "$ZANT_NOTARY_PROFILE")
  elif [ -n "${ZANT_NOTARY_KEY:-}" ]; then
    auth=(--key "$ZANT_NOTARY_KEY" --key-id "$ZANT_NOTARY_KEY_ID" --issuer "$ZANT_NOTARY_ISSUER")
  elif [ -n "${ZANT_NOTARY_APPLE_ID:-}" ]; then
    auth=(--apple-id "$ZANT_NOTARY_APPLE_ID" --password "$ZANT_NOTARY_PASSWORD" --team-id "$ZANT_NOTARY_TEAM")
  else
    die "没有公证凭据"
  fi
}

# Submits a .dmg / .zip and waits for Apple's result; prints the check log
# when it is rejected.
notarize() {
  local file="$1" out err id status
  notary_auth
  step "提交 Apple 公证：$(basename "$file")（上传后等苹果检查，通常 1～10 分钟）"
  err=$(mktemp)
  out=$(xcrun notarytool submit "$file" --wait --output-format json "${auth[@]}" 2>"$err") || true
  id=$(plutil -extract id raw -o - - <<<"$out" 2>/dev/null || true)
  status=$(plutil -extract status raw -o - - <<<"$out" 2>/dev/null || true)
  if [ "$status" != "Accepted" ]; then
    cat "$err" >&2
    rm -f "$err"
    printf '%s\n' "$out" >&2
    if [ -n "$id" ]; then
      echo "  苹果的检查日志：" >&2
      xcrun notarytool log "$id" "${auth[@]}" >&2 || true
    fi
    die "公证没有通过（${status:-提交失败}）"
  fi
  rm -f "$err"
  ok "公证通过（$id）"
}

staple() {
  xcrun stapler staple -q "$1" || die "装订公证票据失败：$1"
}

assess() {
  local result
  if result=$(spctl --assess -vv "$@" 2>&1); then
    ok "Gatekeeper 检查通过：$(printf '%s' "$result" | grep -o 'source=.*' | head -1)"
  else
    printf '%s\n' "$result" >&2
    die "Gatekeeper 检查没有通过"
  fi
}

# A DMG whose window shows the background picture, with the app and the
# Applications link placed on it (icon centres at 27 % / 73 % of the width,
# 47.5 % of the height: where the default background's arrow starts and
# ends). Finder lays the window out and stores it in the image's .DS_Store;
# without permission to control Finder the DMG keeps a plain window.
dmg_with_layout() {
  local stage="$1" name="$2" dmg="$3" work rw attach dev mnt w h bg2x x1 x2 y
  if [ -e "/Volumes/$ZANT_TITLE" ]; then
    die "已经挂载了名为「$ZANT_TITLE」的磁盘：先在访达里推出它再打包"
  fi
  work=$(mktemp -d)
  rw="$work/rw.dmg"
  w=$(sips -g pixelWidth "$ZANT_DMG_BACKGROUND" | awk '/pixelWidth/ {print $2}')
  h=$(sips -g pixelHeight "$ZANT_DMG_BACKGROUND" | awk '/pixelHeight/ {print $2}')
  mkdir "$stage/.background"
  bg2x="${ZANT_DMG_BACKGROUND%.png}@2x.png"
  if [ -f "$bg2x" ]; then
    tiffutil -cathidpicheck "$ZANT_DMG_BACKGROUND" "$bg2x" -out "$stage/.background/background.tiff" 2>/dev/null
  else
    sips -s format tiff "$ZANT_DMG_BACKGROUND" --out "$stage/.background/background.tiff" >/dev/null
  fi
  hdiutil create -quiet -volname "$ZANT_TITLE" -srcfolder "$stage" -fs HFS+ -format UDRW -ov "$rw"
  attach=$(hdiutil attach -readwrite -noverify -noautoopen "$rw" 2> >(grep -v deprecated >&2))
  dev=$(printf '%s\n' "$attach" | awk '/\/Volumes\// {print $1; exit}')
  mnt=$(printf '%s\n' "$attach" | sed -n 's|.*\(/Volumes/.*\)$|\1|p' | head -1)
  [ -n "$dev" ] && [ -n "$mnt" ] || die "无法挂载临时磁盘映像"
  x1=$((w * 27 / 100))
  x2=$((w * 73 / 100))
  y=$((h * 475 / 1000))
  if ! osascript >/dev/null 2>"$work/finder.log" <<APPLESCRIPT
tell application "Finder"
  tell disk "$ZANT_TITLE"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set pathbar visible of container window to false
    set the bounds of container window to {200, 120, 200 + $w, 120 + $h}
    set opts to the icon view options of container window
    set arrangement of opts to not arranged
    set icon size of opts to 128
    set text size of opts to 13
    set background picture of opts to file ".background:background.tiff"
    set position of item "$name" of container window to {$x1, $y}
    set position of item "Applications" of container window to {$x2, $y}
    close
    open
    update without registering applications
    delay 1
    close
  end tell
end tell
APPLESCRIPT
  then
    printf '  \033[33m!\033[0m 没能设置 DMG 窗口（背景和图标位置）：%s\n' "$(tr '\n' ' ' <"$work/finder.log")" >&2
    printf '    需要允许终端控制「访达」：系统设置 → 隐私与安全性 → 自动化\n' >&2
  fi
  # Finder writes .DS_Store a moment later
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    [ -f "$mnt/.DS_Store" ] && break
    sleep 0.5
  done
  rm -rf "$mnt/.fseventsd"
  sync
  for _ in 1 2 3 4 5; do
    hdiutil detach -quiet "$dev" && break
    sleep 1
  done
  hdiutil convert -quiet "$rw" -format UDZO -imagekey zlib-level=9 -o "$dmg"
  rm -rf "$work"
}

developer_id() {
  local base="$ZANT_TITLE-$ZANT_VERSION" name
  name=$(basename "$ZANT_APP")
  mkdir -p "$ZANT_OUT"
  if [ "${ZANT_FORMAT:-dmg}" = "zip" ]; then
    local zip="$ZANT_OUT/$base.zip"
    rm -f "$zip"
    ditto -c -k --keepParent "$ZANT_APP" "$zip"
    if [ "${ZANT_NOTARIZE:-0}" = 1 ]; then
      notarize "$zip"
      staple "$ZANT_APP"
      assess --type exec "$ZANT_APP"
      rm -f "$zip"
      ditto -c -k --keepParent "$ZANT_APP" "$zip" # now with the ticket
    fi
    ok "已生成 $zip"
    return
  fi

  local dmg="$ZANT_OUT/$base.dmg" stage
  stage=$(mktemp -d)
  step "制作 DMG"
  ditto "$ZANT_APP" "$stage/$name"
  ln -s /Applications "$stage/Applications"
  rm -f "$dmg"
  if [ -n "${ZANT_DMG_BACKGROUND:-}" ]; then
    dmg_with_layout "$stage" "$name" "$dmg"
  else
    hdiutil create -quiet -volname "$ZANT_TITLE" -srcfolder "$stage" -fs HFS+ -format UDZO -ov "$dmg"
  fi
  rm -rf "$stage"
  if [ "$ZANT_IDENTITY" != "none" ]; then
    codesign --force --timestamp --sign "$ZANT_IDENTITY" ${keychain_args[@]+"${keychain_args[@]}"} "$dmg"
  fi
  ok "已生成 $dmg"
  if [ "${ZANT_NOTARIZE:-0}" = 1 ]; then
    notarize "$dmg"
    staple "$dmg"
    staple "$ZANT_APP" # the ticket covers the app inside too
    assess --type open --context context:primary-signature "$dmg"
  fi
}

app_store() {
  local pkg="$ZANT_OUT/$ZANT_TITLE-$ZANT_VERSION.pkg"
  mkdir -p "$ZANT_OUT"
  rm -f "$pkg"
  step "制作安装包 .pkg（$ZANT_INSTALLER_IDENTITY）"
  productbuild --component "$ZANT_APP" /Applications --sign "$ZANT_INSTALLER_IDENTITY" ${keychain_args[@]+"${keychain_args[@]}"} "$pkg" >/dev/null
  ok "已生成 $pkg"
  if [ "${ZANT_UPLOAD:-0}" = 1 ]; then
    local auth=(--api-key "$ZANT_ASC_KEY_ID" --api-issuer "$ZANT_ASC_ISSUER" --p8-file-path "$ZANT_ASC_KEY")
    step "App Store Connect 验证"
    xcrun altool --validate-app "$pkg" "${auth[@]}" || die "App Store Connect 验证没有通过（见上面的错误）"
    step "上传到 App Store Connect"
    xcrun altool --upload-package "$pkg" "${auth[@]}" || die "上传失败（见上面的错误）"
    ok "已上传：苹果处理完成后（通常 10～30 分钟）在 App Store Connect 的 TestFlight 里可以看到这个版本"
  fi
}

# CI: a temporary keychain with the .p12, put first in the search list (the
# old list is saved next to it and restored by remove-keychain).
import_cert() {
  local kc="$ZANT_KEYCHAIN" pw
  pw=$(openssl rand -hex 24)
  security delete-keychain "$kc" 2>/dev/null || true
  security list-keychains -d user | sed -e 's/^ *"//' -e 's/"$//' >"$kc.search"
  security create-keychain -p "$pw" "$kc"
  security set-keychain-settings -lut 21600 "$kc"
  security unlock-keychain -p "$pw" "$kc"
  security import "$ZANT_CERT" -k "$kc" -P "${ZANT_CERT_PASSWORD:-}" -T /usr/bin/codesign -T /usr/bin/productbuild >/dev/null ||
    die "导入证书失败（检查证书文件和密码）"
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$pw" "$kc" >/dev/null
  local list=()
  while IFS= read -r line; do [ -n "$line" ] && list+=("$line"); done <"$kc.search"
  security list-keychains -d user -s "$kc" "${list[@]}"
  ok "证书已导入临时钥匙串（构建结束后删除）"
}

remove_keychain() {
  local kc="$ZANT_KEYCHAIN"
  if [ -f "$kc.search" ]; then
    local list=()
    while IFS= read -r line; do [ -n "$line" ] && list+=("$line"); done <"$kc.search"
    security list-keychains -d user -s "${list[@]}"
    rm -f "$kc.search"
  fi
  security delete-keychain "$kc" 2>/dev/null || true
}

case "${1:-}" in
developer-id) developer_id ;;
app-store) app_store ;;
import-cert) import_cert ;;
remove-keychain) remove_keychain ;;
*) die "用法: macos.sh developer-id | app-store | import-cert | remove-keychain" ;;
esac
