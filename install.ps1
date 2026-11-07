# Installs Zant UI and its `zant` CLI on Windows 10/11 x64 (PowerShell):
#
#   irm https://raw.githubusercontent.com/racall/zant-ui/main/install.ps1 | iex
#
# 1. Zig: uses `zig` from PATH when it is the version Zant UI needs,
#    otherwise downloads the official build into $ZANT_HOME\zig.
# 2. Git and Visual Studio Build Tools (C++ workload): offers to install
#    them with winget when missing.
# 3. Clones Zant UI into $ZANT_HOME\zant-ui (or updates it).
# 4. Downloads the pinned Skia, PDFium and fonts (scripts/fetch_skia_windows.ps1).
# 5. Builds zant.exe and adds its folder to the user PATH.
#
# Running it again updates everything (`zant update` does the same for the
# code). Settings (environment variables, set before running):
#   ZANT_HOME   install folder (default %LOCALAPPDATA%\zant)
#   ZANT_REPO   git URL (default https://github.com/racall/zant-ui.git)
#   ZANT_REF    branch or tag (default main)
#   ZANT_YES=1  install missing tools without asking
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$ZigVersion = '0.17.1'
$ZigUrl = "https://ziglang.org/download/$ZigVersion/zig-x86_64-windows-$ZigVersion.zip"
$ZigSha = 'b03f477a1943673e02aca0f570e8030dffd536ce1f45cbc4d3326ec31159562e'

$ZantHome = if ($env:ZANT_HOME) { $env:ZANT_HOME } else { Join-Path $env:LOCALAPPDATA 'zant' }
$Repo = if ($env:ZANT_REPO) { $env:ZANT_REPO } else { 'https://github.com/racall/zant-ui.git' }
$Ref = if ($env:ZANT_REF) { $env:ZANT_REF } else { 'main' }
$Src = Join-Path $ZantHome 'zant-ui'

function Step($t) { Write-Host '  → ' -ForegroundColor Cyan -NoNewline; Write-Host $t }
function Ok($t) { Write-Host '  ✓ ' -ForegroundColor Green -NoNewline; Write-Host $t }
# throw, not exit: under `irm | iex` exit would close the user's PowerShell
function Die($t) { Write-Host ''; Write-Host '  ✗ ' -ForegroundColor Red -NoNewline; Write-Host $t; Write-Host ''; throw 'Zant UI 安装没有完成' }
function Ask($question) {
    if ($env:ZANT_YES -eq '1') { return $true }
    $a = Read-Host "$question [Y/n]"
    return ($a -eq '' -or $a -match '^[yY]')
}
function Refresh-Path {
    $env:PATH = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
}
function Find-VC {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (!(Test-Path $vswhere)) { return $null }
    $path = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($path) { return $path } else { return $null }
}

Write-Host ''
Write-Host 'Zant UI 安装' -ForegroundColor Cyan -NoNewline; Write-Host "（$ZantHome）"

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

# ---- 2. Windows, Git, C++ build tools
if (![Environment]::Is64BitOperatingSystem -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
    Die 'Zant UI 在 Windows 上需要 x64（预编译的 Skia 只有 x64 版本）'
}
if (!(Get-Command git -ErrorAction SilentlyContinue)) {
    if (!(Ask '没有找到 Git。现在用 winget 安装吗？')) { Die '需要 Git：winget install --id Git.Git -e' }
    winget install --id Git.Git -e --accept-package-agreements --accept-source-agreements
    Refresh-Path
    if (!(Get-Command git -ErrorAction SilentlyContinue)) { Die 'Git 安装后仍找不到：重新打开 PowerShell 再运行' }
}
if (!(Find-VC)) {
    Step '没有找到 Visual Studio 的 C++ 生成工具（编译 Skia 桥接库要用，约 3～6 GB，需要管理员确认）'
    if (!(Ask '现在用 winget 安装 Visual Studio Build Tools 吗？')) {
        Die '需要 Visual Studio 2022+ 或 Build Tools 的「使用 C++ 的桌面开发」工作负载'
    }
    winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-package-agreements --accept-source-agreements `
        --override '--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
    if (!(Find-VC)) { Die 'C++ 生成工具安装后仍找不到：在 Visual Studio Installer 里确认勾选了「使用 C++ 的桌面开发」' }
}
Ok 'Git 与 C++ 生成工具'

if (!$Zig) {
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("zant-zig-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        $zip = Join-Path $tmp 'zig.zip'
        curl.exe -fL --retry 3 -o $zip $ZigUrl
        if ($LASTEXITCODE -ne 0) { Die "下载 Zig 失败：$ZigUrl" }
        $actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
        if ($actual -ne $ZigSha) { Die "Zig 下载的文件校验不对（$actual），请重试" }
        tar.exe -xf $zip -C $tmp
        if ($LASTEXITCODE -ne 0) { Die '解压 Zig 失败' }
        $dest = Join-Path $ZantHome "zig\$ZigVersion"
        if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
        New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
        Move-Item (Join-Path $tmp "zig-x86_64-windows-$ZigVersion") $dest
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
    $Zig = $local
    Ok "Zig ${ZigVersion}：$Zig"
}

# ---- 3. the source
if (Test-Path (Join-Path $Src '.git')) {
    Step "更新 Zant UI（$Ref）"
    git -C $Src fetch --quiet origin $Ref
    git -C $Src checkout --quiet $Ref
    git -C $Src merge --quiet --ff-only FETCH_HEAD
    if ($LASTEXITCODE -ne 0) { Die "$Src 有本地修改，不能自动更新：处理后再运行" }
} else {
    if (Test-Path $Src) { Die "$Src 已存在但不是 git 仓库：移走后再运行" }
    Step "下载 Zant UI（$Ref）"
    New-Item -ItemType Directory -Force $ZantHome | Out-Null
    git clone --quiet --branch $Ref $Repo $Src
    if ($LASTEXITCODE -ne 0) { Die "git clone 失败：$Repo" }
}
Ok "Zant UI：$Src（$((git -C $Src rev-parse --short HEAD).Trim())）"

# ---- 4. Skia, PDFium, fonts
Step '下载 Skia 等依赖（只在缺少或版本变化时下载）'
powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $Src 'scripts\fetch_skia_windows.ps1')
if ($LASTEXITCODE -ne 0) { Die '下载依赖失败（见上面的错误）' }

# ---- 5. zant
Step '编译 zant（第一次要几分钟）'
$bin = Join-Path $Src 'zig-out\bin'
$exe = Join-Path $bin 'zant.exe'
if (Test-Path $exe) {
    # a running zant.exe cannot be replaced, but it can be renamed
    Remove-Item (Join-Path $bin 'zant.old.exe') -Force -ErrorAction SilentlyContinue
    Rename-Item $exe 'zant.old.exe' -ErrorAction SilentlyContinue
}
Push-Location $Src
try { & $Zig build cli } finally { Pop-Location }
if ($LASTEXITCODE -ne 0) { Die '编译 zant 失败（见上面的错误）' }
Ok "zant：$exe"

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$parts = @($userPath -split ';' | Where-Object { $_ })
$add = @($bin)
if (!(Get-Command zig -ErrorAction SilentlyContinue)) { $add += (Split-Path $Zig) }
$changed = $false
foreach ($dir in $add) {
    if ($parts -notcontains $dir) { $parts += $dir; $changed = $true }
}
if ($changed) {
    [Environment]::SetEnvironmentVariable('Path', ($parts -join ';'), 'User')
    Ok '已加到用户 PATH，新开的终端里生效'
}
Refresh-Path

Write-Host ''
Write-Host '安装完成' -ForegroundColor Green -NoNewline; Write-Host "。打开新的终端试试：`n"
Write-Host '  zant create my-app'
Write-Host "  cd my-app; zant run`n"
Write-Host "以后更新：zant update`n"
