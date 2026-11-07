# Installs the zant CLI and Zant UI on Windows 10/11 x64, ready to use:
#
#   irm https://raw.githubusercontent.com/racall/zant-ui/main/install.ps1 | iex
#
# (raw.githubusercontent.com serves it as UTF-8; release assets come as
# application/octet-stream, which `irm` would decode as Latin-1)
#
# 1. Zig: uses `zig` from PATH when it is the version Zant UI needs,
#    otherwise downloads the official build into $ZANT_HOME\zig.
# 2. Downloads the release archive into $ZANT_HOME: zant.exe, Zant UI, Skia,
#    PDFium and the prebuilt Skia bridge with the MSVC runtime, so nothing is
#    compiled and no Visual Studio is needed. Adds $ZANT_HOME\bin to the
#    user PATH.
#
# Running it again (or `zant update`) updates to the latest release.
# Settings (environment variables, set before running):
#   ZANT_HOME      install folder (default %LOCALAPPDATA%\zant)
#   ZANT_VERSION   a release tag, e.g. v0.1.0 (default: the latest)
#   ZANT_DOWNLOAD  where the release files are (default: GitHub Releases)
#   ZANT_FORCE=1   download again even when up to date
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$ZigVersion = '0.17.1'
$ZigUrl = "https://ziglang.org/download/$ZigVersion/zig-x86_64-windows-$ZigVersion.zip"
$ZigSha = 'b03f477a1943673e02aca0f570e8030dffd536ce1f45cbc4d3326ec31159562e'

$ZantHome = if ($env:ZANT_HOME) { $env:ZANT_HOME } else { Join-Path $env:LOCALAPPDATA 'zant' }
$Download = if ($env:ZANT_DOWNLOAD) { $env:ZANT_DOWNLOAD }
            elseif ($env:ZANT_VERSION) { "https://github.com/racall/zant-ui/releases/download/$env:ZANT_VERSION" }
            else { 'https://github.com/racall/zant-ui/releases/latest/download' }
$Archive = 'zant-windows-x64.zip'

function Step($t) { Write-Host '  → ' -ForegroundColor Cyan -NoNewline; Write-Host $t }
function Ok($t) { Write-Host '  ✓ ' -ForegroundColor Green -NoNewline; Write-Host $t }
# throw, not exit: under `irm | iex` exit would close the user's PowerShell
function Die($t) { Write-Host ''; Write-Host '  ✗ ' -ForegroundColor Red -NoNewline; Write-Host $t; Write-Host ''; throw 'zant 安装没有完成' }
# small files (version, checksum) get a total time limit, archives only a
# connect timeout
function Fetch([string]$Url, [string]$Path, [switch]$Small) {
    $limit = if ($Small) { @('--max-time', '60') } else { @() }
    curl.exe -fsSL --retry 3 --connect-timeout 20 @limit -o $Path $Url
    return ($LASTEXITCODE -eq 0)
}

Write-Host ''
Write-Host '安装 zant' -ForegroundColor Cyan -NoNewline; Write-Host "（$ZantHome）"

$tmp = Join-Path ([IO.Path]::GetTempPath()) ('zant-install-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    # ---- 1. Zig
    $Zig = $null
    $onPath = Get-Command zig -ErrorAction SilentlyContinue
    $local = Join-Path $ZantHome "zig\$ZigVersion\zig.exe"
    if ($onPath -and ((& $onPath.Source version) -eq $ZigVersion)) {
        $Zig = $onPath.Source
        Ok "Zig ${ZigVersion}：$Zig"
    } elseif (Test-Path $local) {
        $Zig = $local
        Ok "Zig ${ZigVersion}：$Zig"
    } elseif ($onPath) {
        Step "已安装的 Zig 是 $(& $onPath.Source version)，Zant UI 需要 ${ZigVersion}：另外下载一份给 zant 用（不影响已有的）"
    } else {
        Step "没有找到 Zig：下载官方的 Zig $ZigVersion"
    }

    # ---- 2. Windows
    if (![Environment]::Is64BitOperatingSystem -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
        Die 'Zant UI 在 Windows 上需要 x64（预编译的 Skia 只有 x64 版本）'
    }

    if (!$Zig) {
        $zip = Join-Path $tmp 'zig.zip'
        if (!(Fetch $ZigUrl $zip)) { Die "下载 Zig 失败：$ZigUrl" }
        $actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
        if ($actual -ne $ZigSha) { Die "Zig 下载的文件校验不对（$actual），请重试" }
        tar.exe -xf $zip -C $tmp
        if ($LASTEXITCODE -ne 0) { Die '解压 Zig 失败' }
        $dest = Join-Path $ZantHome "zig\$ZigVersion"
        if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
        New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
        Move-Item (Join-Path $tmp "zig-x86_64-windows-$ZigVersion") $dest
        $Zig = $local
        Ok "Zig ${ZigVersion}：$Zig"
    }

    # ---- 3. zant and Zant UI
    $verFile = Join-Path $tmp 'version.txt'
    if (!(Fetch "$Download/version.txt" $verFile -Small)) { Die "无法获取版本信息：$Download/version.txt" }
    $latest = (Get-Content -Raw $verFile).Trim()
    $ui = Join-Path $ZantHome 'zant-ui'
    $bin = Join-Path $ZantHome 'bin'
    $exe = Join-Path $bin 'zant.exe'
    $installed = if (Test-Path (Join-Path $ui '.zant-version')) { (Get-Content -Raw (Join-Path $ui '.zant-version')).Trim() } else { '' }
    if ($installed -eq $latest -and (Test-Path $exe) -and $env:ZANT_FORCE -ne '1') {
        Ok "zant $latest 已经是最新"
    } else {
        if ($installed) { Step "更新 zant：$installed → $latest" } else { Step "下载 zant $latest" }
        $zip = Join-Path $tmp $Archive
        if (!(Fetch "$Download/$Archive" $zip)) { Die "下载失败：$Download/$Archive" }
        $shaFile = Join-Path $tmp 'archive.sha256'
        if (!(Fetch "$Download/$Archive.sha256" $shaFile -Small)) { Die '下载校验值失败' }
        if ((Get-FileHash $zip -Algorithm SHA256).Hash.ToLower() -ne (Get-Content -Raw $shaFile).Trim()) { Die '下载的文件校验不对，请重试' }
        $new = Join-Path $tmp 'new'
        New-Item -ItemType Directory -Path $new | Out-Null
        tar.exe -xf $zip -C $new
        if ($LASTEXITCODE -ne 0) { Die '解压失败' }
        if (!(Test-Path (Join-Path $new 'bin\zant.exe')) -or !(Test-Path (Join-Path $new 'zant-ui\build.zig.zon'))) { Die '发布包内容不完整' }

        # swap in the new version (projects refer to $ZANT_HOME\zant-ui, so the path stays)
        New-Item -ItemType Directory -Force $bin | Out-Null
        $old = Join-Path $ZantHome 'zant-ui.old'
        if (Test-Path $old) { Remove-Item -Recurse -Force $old }
        if (Test-Path $ui) {
            try { Rename-Item $ui 'zant-ui.old' } catch { Die "无法替换 ${ui}：关掉正在使用它的终端或编辑器后再试" }
        }
        Move-Item (Join-Path $new 'zant-ui') $ui
        if (Test-Path $exe) {
            # a running zant.exe (zant update) cannot be replaced, but it can be renamed
            Remove-Item (Join-Path $bin 'zant.old.exe') -Force -ErrorAction SilentlyContinue
            Rename-Item $exe 'zant.old.exe'
        }
        Move-Item (Join-Path $new 'bin\zant.exe') $exe
        Remove-Item -Recurse -Force $old -ErrorAction SilentlyContinue
        Ok "zant ${latest}：$ZantHome"
    }

    # ---- 4. PATH
    $parts = @(([Environment]::GetEnvironmentVariable('Path', 'User')) -split ';' | Where-Object { $_ })
    $add = @($bin)
    if (!(Get-Command zig -ErrorAction SilentlyContinue)) { $add += (Split-Path $Zig) }
    $changed = $false
    foreach ($dir in $add) { if ($parts -notcontains $dir) { $parts += $dir; $changed = $true } }
    if ($changed) {
        [Environment]::SetEnvironmentVariable('Path', ($parts -join ';'), 'User')
        Ok '已加到用户 PATH，新开的终端里生效'
    }
    foreach ($dir in $add) { if (($env:PATH -split ';') -notcontains $dir) { $env:PATH = "$dir;$env:PATH" } }
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host ''
Write-Host '完成' -ForegroundColor Green -NoNewline; Write-Host '。试试：'
Write-Host ''
Write-Host '  zant create my-app'
Write-Host '  cd my-app; zant run'
Write-Host ''
Write-Host '以后更新：zant update'
Write-Host ''
