# Installs the zant CLI and Zant UI on Windows 10/11 x64, ready to use:
#
#   irm https://raw.githubusercontent.com/racall/zant-ui/main/install.ps1 | iex
#
# (raw.githubusercontent.com serves it as UTF-8; release assets come as
# application/octet-stream, which `irm` would decode as Latin-1)
#
# 1. Rust: installs rustup into %USERPROFILE%\.cargo when it is missing and
#    makes sure the Rust toolchain Zant UI pins (zant-ui\rust-toolchain.toml)
#    is installed. Also checks Visual Studio Build Tools (the MSVC linker and
#    Windows SDK cargo needs).
# 2. Downloads the release archive into $ZANT_HOME: zant.exe, the Rust
#    workspace with the native sources, Skia, PDFium and fonts, so nothing is
#    compiled during installation. Adds $ZANT_HOME\bin to the user PATH.
#    The archive also contains a prebuilt native library
#    (zant-ui\third_party\zant-native-prebuilt\, made by
#    scripts/release/prebuilt.ps1). Apps built from this install link it
#    instead of compiling C++/C — Visual Studio Build Tools stay required to
#    link, and the sources are still shipped so a stale archive falls back to
#    compiling.
#
# Running it again (or `zant update`) updates to the latest release.
# Settings (environment variables, set before running):
#   ZANT_HOME      install folder (default %LOCALAPPDATA%\zant)
#   ZANT_VERSION   a release tag, e.g. v0.1.0 (default: the latest)
#   ZANT_DOWNLOAD  where the release files are (default: GitHub Releases)
#   ZANT_FORCE=1   download again even when up to date
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# rustup-init for x64; the .sha256 next to it is published by
# static.rust-lang.org. The version of rustup itself is not pinned (Rust's
# download server has no per-version paths); the *toolchain* below is.
$RustupTarget = 'x86_64-pc-windows-msvc'
$RustupUrl = "https://static.rust-lang.org/rustup/dist/$RustupTarget/rustup-init.exe"
# fallback when zant-ui\rust-toolchain.toml cannot be read
$FallbackToolchain = 'stable'
# the components rust-toolchain.toml asks for (rustup does not install them
# with --profile minimal)
$ToolchainComponents = @('rustfmt', 'clippy')

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

# Native tools' stderr is not turned into PowerShell errors here (Windows
# PowerShell 5.1 would, under 'Stop'); their exit codes are checked instead.
$tmp = Join-Path ([IO.Path]::GetTempPath()) ('zant-install-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null
$newUi = $null
try {
    # ---- 1. Windows: x64 and the C++ build tools cargo links with
    if (![Environment]::Is64BitOperatingSystem -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
        Die 'Zant UI 在 Windows 上需要 x64（预编译的 Skia 只有 x64 版本）'
    }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $vs = $null
    if (Test-Path $vswhere) {
        $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
    }
    if (!$vs) {
        Die '需要 Visual Studio Build Tools（「使用 C++ 的桌面开发」工作负载，含 Windows SDK）来编译应用：安装后重新运行这条命令。安装：winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"'
    }
    Ok "Visual Studio：$vs"

    # ---- 2. Rust: rustup (installed here when missing) + the pinned toolchain
    # `zant update` can run this script from a shell where %USERPROFILE%\.cargo\bin
    # is not on PATH yet, so look there too.
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    if (Test-Path (Join-Path $cargoBin 'rustup.exe')) { $env:PATH = "$cargoBin;$env:PATH" }
    if (Get-Command rustup -ErrorAction SilentlyContinue) {
        Ok "rustup：$((Get-Command rustup).Source)"
    } else {
        Step '没有找到 rustup：下载 Rust 安装器（rustup-init）'
        $init = Join-Path $tmp 'rustup-init.exe'
        if (!(Fetch $RustupUrl $init)) { Die "下载 Rust 安装器失败：$RustupUrl" }
        $shaFile = Join-Path $tmp 'rustup-init.sha256'
        if (!(Fetch "$RustupUrl.sha256" $shaFile -Small)) { Die '下载 Rust 安装器的校验值失败' }
        $expected = (((Get-Content -Raw $shaFile).Trim()) -split '\s+')[0]
        if ((Get-FileHash $init -Algorithm SHA256).Hash.ToLower() -ne $expected.ToLower()) { Die 'Rust 安装器校验不对，请重试' }
        # -y: no questions; --default-toolchain none: the pinned channel is
        # installed below, once the release is unpacked
        & $init -y --profile minimal --default-toolchain none
        if ($LASTEXITCODE -ne 0) { Die '安装 rustup 失败（见上面的错误）' }
        $env:PATH = "$cargoBin;$env:PATH"
        if (!(Get-Command rustup -ErrorAction SilentlyContinue)) { Die '安装后仍找不到 rustup：重新打开终端后再运行这条命令' }
        Ok "rustup：$((Get-Command rustup).Source)"
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
        if ((Get-FileHash $zip -Algorithm SHA256).Hash.ToLower() -ne ((Get-Content -Raw $shaFile).Trim())) { Die '下载的文件校验不对，请重试' }
        $new = Join-Path $tmp 'new'
        New-Item -ItemType Directory -Path $new | Out-Null
        tar.exe -xf $zip -C $new
        if ($LASTEXITCODE -ne 0) { Die '解压失败' }
        $newUi = Join-Path $new 'zant-ui'
        if (!(Test-Path (Join-Path $new 'bin\zant.exe')) -or !(Test-Path (Join-Path $newUi 'Cargo.toml'))) { Die '发布包内容不完整' }
    }

    # ---- 4. the Rust toolchain Zant UI pins (zant-ui\rust-toolchain.toml)
    $pinDir = if ($newUi) { $newUi } else { $ui }
    $channel = ''
    $pinFile = Join-Path $pinDir 'rust-toolchain.toml'
    if (Test-Path $pinFile) {
        $found = Get-Content $pinFile | Select-String -Pattern '^\s*channel\s*=\s*"([^"]+)"' | Select-Object -First 1
        if ($found) { $channel = $found.Matches[0].Groups[1].Value }
    }
    if (!$channel) { $channel = $FallbackToolchain }
    $components = @()
    foreach ($component in $ToolchainComponents) { $components += @('--component', $component) }
    Step "确保 Rust 工具链 $channel（编译应用用；第一次要下载几百 MB）"
    & rustup toolchain install $channel --profile minimal @components
    if ($LASTEXITCODE -ne 0) { Die "安装 Rust 工具链失败：rustup toolchain install $channel" }
    Ok "Rust 工具链：$channel"
    # a fresh rustup has no default toolchain: apps run cargo outside zant-ui too
    if (!((& rustup toolchain list 2>$null) -match '\(default\)')) { & rustup default $channel | Out-Null }
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (!$cargo) { Die "找不到 cargo：确认 $cargoBin 在 PATH 上（重新打开终端后再试）" }
    # run it outside the caller's directory: a rust-toolchain.toml there would
    # make the rustup shim pick (and try to install) that project's toolchain
    Push-Location $tmp
    try { $cargoVersion = & $cargo.Source --version } finally { Pop-Location }
    Ok $cargoVersion

    # swap in the new version (projects refer to $ZANT_HOME\zant-ui, so the path stays)
    if ($newUi) {
        New-Item -ItemType Directory -Force $bin | Out-Null
        $old = Join-Path $ZantHome 'zant-ui.old'
        if (Test-Path $old) { Remove-Item -Recurse -Force $old }
        if (Test-Path $ui) {
            try { Rename-Item $ui 'zant-ui.old' } catch { Die "无法替换 ${ui}：关掉正在使用它的终端或编辑器后再试" }
        }
        Move-Item $newUi $ui
        if (Test-Path $exe) {
            # a running zant.exe (zant update) cannot be replaced, but it can be renamed
            Remove-Item (Join-Path $bin 'zant.old.exe') -Force -ErrorAction SilentlyContinue
            Rename-Item $exe 'zant.old.exe'
        }
        Move-Item (Join-Path $new 'bin\zant.exe') $exe
        Remove-Item -Recurse -Force $old -ErrorAction SilentlyContinue
        Ok "zant ${latest}：$ZantHome"
    }

    # ---- 5. PATH
    $parts = @(([Environment]::GetEnvironmentVariable('Path', 'User')) -split ';' | Where-Object { $_ })
    $add = @($bin, $cargoBin) | Where-Object { Test-Path $_ } | Select-Object -Unique
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
