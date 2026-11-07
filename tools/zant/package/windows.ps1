# Windows release steps run by `zant build` after it has compiled the app
# into zig-out/<title>/ (the exe, zant_skia.dll, pdfium.dll, MSVC runtime).
# zant asks for and resolves the settings (build.config.json, environment
# variables) and passes them as ZANT_* environment variables; passwords never
# appear on a command line or in the log.
#
#   windows.ps1 list-certs     code signing certificates: thumbprint|subject|expiry
#   windows.ps1 find-iscc      path of Inno Setup's ISCC.exe (empty if missing)
#   windows.ps1 install-iscc   install Inno Setup with winget
#   windows.ps1 package        sign, then build ZANT_KIND = installer | store | folder
#
# package: ZANT_TITLE, ZANT_VERSION, ZANT_BUNDLE_ID, ZANT_DIR (app folder),
#   ZANT_OUT (output folder), ZANT_KIND
#   signing (optional): ZANT_SIGN_THUMBPRINT | ZANT_SIGN_SUBJECT |
#     ZANT_SIGN_PFX + ZANT_SIGN_PFX_PASSWORD; ZANT_SIGN_TIMESTAMP
#   installer: ZANT_PUBLISHER, ZANT_ICO (optional)
#   store: ZANT_MSIX_NAME, ZANT_MSIX_PUBLISHER, ZANT_MSIX_PUBLISHER_NAME,
#     ZANT_MSIX_VERSION, ZANT_MSIX_ASSETS (folder with the logos),
#     ZANT_STORE_UPLOAD (1 | 0), ZANT_STORE_PRODUCT_ID and optionally
#     ZANT_STORE_TENANT_ID / _SELLER_ID / _CLIENT_ID / _CLIENT_SECRET
param([Parameter(Mandatory = $true)][string]$Action)
# native tools' stderr is never redirected here (Windows PowerShell 5.1 would
# turn it into errors under 'Stop'); their exit codes are checked instead
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$esc = [char]27

function Step($text) { [Console]::Error.WriteLine("  $esc[36m→$esc[0m $text") }
function Ok($text) { [Console]::Error.WriteLine("  $esc[32m✓$esc[0m $text") }
function Die($text) { [Console]::Error.WriteLine("`n  $esc[31m✗$esc[0m $text`n"); exit 1 }

function Run($exe, [string[]]$arguments, $what) {
    & $exe @arguments
    if ($LASTEXITCODE -ne 0) { Die "$what 失败（退出码 $LASTEXITCODE）" }
}

# signtool.exe / makeappx.exe from the newest Windows 10/11 SDK
function Find-SdkTool($name) {
    $override = [Environment]::GetEnvironmentVariable("ZANT_" + ($name -replace '\.exe$', '').ToUpper())
    if ($override) { return $override }
    $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    if (Test-Path $kits) {
        $found = Get-ChildItem $kits -Directory | Where-Object { $_.Name -match '^10\.' } |
            Sort-Object { [version]$_.Name } -Descending |
            ForEach-Object { Join-Path $_.FullName "x64\$name" } | Where-Object { Test-Path $_ } | Select-Object -First 1
        if ($found) { return $found }
    }
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    Die "找不到 ${name}：在 Visual Studio Installer 里安装「Windows 11 SDK」组件，或设置 ZANT_$((($name -replace '\.exe$', '').ToUpper()))"
}

function Find-Iscc {
    if ($env:ZANT_ISCC) { return $env:ZANT_ISCC }
    $cmd = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    foreach ($base in @(${env:ProgramFiles(x86)}, $env:ProgramFiles, (Join-Path $env:LOCALAPPDATA 'Programs'))) {
        if (-not $base) { continue }
        foreach ($v in @('Inno Setup 7', 'Inno Setup 6')) {
            $p = Join-Path $base "$v\ISCC.exe"
            if (Test-Path $p) { return $p }
        }
    }
    return ''
}

function Sign-Args {
    $a = @('sign', '/fd', 'sha256', '/tr', $env:ZANT_SIGN_TIMESTAMP, '/td', 'sha256')
    if ($env:ZANT_SIGN_PFX) { $a += @('/f', $env:ZANT_SIGN_PFX); if ($env:ZANT_SIGN_PFX_PASSWORD) { $a += @('/p', $env:ZANT_SIGN_PFX_PASSWORD) } }
    elseif ($env:ZANT_SIGN_THUMBPRINT) { $a += @('/sha1', $env:ZANT_SIGN_THUMBPRINT) }
    elseif ($env:ZANT_SIGN_SUBJECT) { $a += @('/n', $env:ZANT_SIGN_SUBJECT) }
    return $a
}

function Signing { return [bool]($env:ZANT_SIGN_PFX -or $env:ZANT_SIGN_THUMBPRINT -or $env:ZANT_SIGN_SUBJECT) }

function Sign-Files([string[]]$files) {
    $tool = Find-SdkTool 'signtool.exe'
    $sign = @(Sign-Args) + $files
    & $tool @sign | Out-Null
    if ($LASTEXITCODE -ne 0) { Die "签名失败（signtool 退出码 $LASTEXITCODE；检查证书是否可用、时间戳服务器能否访问）" }
}

function Installer {
    $iscc = Find-Iscc
    if (-not $iscc) { Die '找不到 Inno Setup（ISCC.exe）：winget install JRSoftware.InnoSetup，或设置 ZANT_ISCC' }
    $template = Join-Path $PSScriptRoot 'windows-installer.iss'
    $name = "$env:ZANT_TITLE-$env:ZANT_VERSION-setup"
    $numeric = ($env:ZANT_VERSION -split '[^0-9.]')[0].Trim('.')
    if (-not $numeric) { $numeric = '0.0.0' }
    $defines = @(
        "/DAppTitle=$env:ZANT_TITLE", "/DAppVersion=$env:ZANT_VERSION", "/DFileVersion=$numeric",
        "/DAppId=$env:ZANT_BUNDLE_ID", "/DPublisher=$env:ZANT_PUBLISHER",
        "/DSourceDir=$env:ZANT_DIR", "/DOutputDir=$env:ZANT_OUT", "/DOutputName=$name"
    )
    if ($env:ZANT_ICO) { $defines += "/DIconFile=$env:ZANT_ICO" }
    # Simplified Chinese: Inno Setup's own copy if it has one, else the
    # translation shipped next to this script (from Inno Setup's repository)
    $zh = Join-Path (Split-Path $iscc) 'Languages\ChineseSimplified.isl'
    if (-not (Test-Path $zh)) { $zh = Join-Path $PSScriptRoot 'ChineseSimplified.isl' }
    if (Test-Path $zh) { $defines += "/DChinese=$zh" }
    if (Signing) {
        # Inno Setup signs the setup and uninstaller with this command; the
        # password stays an environment variable (cmd expands it), so ISCC's
        # log never shows it
        $tool = Find-SdkTool 'signtool.exe'
        $cmd = '$q' + $tool + '$q sign /fd sha256 /tr ' + $env:ZANT_SIGN_TIMESTAMP + ' /td sha256 '
        if ($env:ZANT_SIGN_PFX) {
            $cmd += '/f $q' + $env:ZANT_SIGN_PFX + '$q '
            if ($env:ZANT_SIGN_PFX_PASSWORD) { $cmd += '/p %ZANT_SIGN_PFX_PASSWORD% ' }
        } elseif ($env:ZANT_SIGN_THUMBPRINT) { $cmd += '/sha1 ' + $env:ZANT_SIGN_THUMBPRINT + ' ' }
        else { $cmd += '/n $q' + $env:ZANT_SIGN_SUBJECT + '$q ' }
        $defines += @('/DSign', ('/Szant=cmd.exe /c $q' + $cmd + '$f$q'))
    }
    Step '制作安装包（Inno Setup）'
    & $iscc '/Q' @defines $template
    if ($LASTEXITCODE -ne 0) { Die "Inno Setup 编译失败（退出码 $LASTEXITCODE）" }
    Ok "已生成 $(Join-Path $env:ZANT_OUT "$name.exe")"
}

function Msix {
    $layout = Join-Path (Split-Path $env:ZANT_OUT) 'build\msix'
    if (Test-Path $layout) { Remove-Item $layout -Recurse -Force }
    New-Item -ItemType Directory -Path $layout | Out-Null
    Copy-Item (Join-Path $env:ZANT_DIR '*') $layout -Recurse
    Copy-Item $env:ZANT_MSIX_ASSETS (Join-Path $layout 'Assets') -Recurse
    $x = { param($s) [System.Security.SecurityElement]::Escape($s) }
    $title = & $x $env:ZANT_TITLE
    $manifest = @"
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
         IgnorableNamespaces="uap rescap">
  <Identity Name="$(& $x $env:ZANT_MSIX_NAME)" Publisher="$(& $x $env:ZANT_MSIX_PUBLISHER)" Version="$env:ZANT_MSIX_VERSION" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>$title</DisplayName>
    <PublisherDisplayName>$(& $x $env:ZANT_MSIX_PUBLISHER_NAME)</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Resources>
    <Resource Language="zh-CN" />
    <Resource Language="en-US" />
  </Resources>
  <Applications>
    <Application Id="App" Executable="$title.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="$title" Description="$title" BackgroundColor="transparent"
                          Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png" />
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
"@
    [System.IO.File]::WriteAllText((Join-Path $layout 'AppxManifest.xml'), $manifest, (New-Object System.Text.UTF8Encoding $false))
    $msix = Join-Path $env:ZANT_OUT "$env:ZANT_TITLE-$env:ZANT_VERSION.msix"
    if (Test-Path $msix) { Remove-Item $msix -Force }
    Step '制作 MSIX 包'
    $tool = Find-SdkTool 'makeappx.exe'
    $log = & $tool pack /o /d $layout /p $msix
    if ($LASTEXITCODE -ne 0) { $log | ForEach-Object { [Console]::Error.WriteLine($_) }; Die "makeappx 打包失败（退出码 $LASTEXITCODE）" }
    if (Signing) {
        Step '签名 MSIX（证书主题要和 Publisher 完全一致）'
        Sign-Files @($msix)
    }
    Ok "已生成 $msix"
    if ($env:ZANT_STORE_UPLOAD -eq '1') {
        $store = Get-Command msstore -ErrorAction SilentlyContinue
        if (-not $store) { Die '找不到 msstore（Microsoft Store Developer CLI）：winget install "Microsoft Store Developer CLI"' }
        if ($env:ZANT_STORE_CLIENT_SECRET) {
            Step '配置 Partner Center 凭据'
            & $store.Source reconfigure --tenantId $env:ZANT_STORE_TENANT_ID --sellerId $env:ZANT_STORE_SELLER_ID `
                --clientId $env:ZANT_STORE_CLIENT_ID --clientSecret $env:ZANT_STORE_CLIENT_SECRET | Out-Null
            if ($LASTEXITCODE -ne 0) { Die 'msstore reconfigure 失败（检查租户、卖家、客户端 ID 和密钥）' }
        }
        Step "提交到 Microsoft Store（产品 $env:ZANT_STORE_PRODUCT_ID）"
        Run $store.Source @('publish', $msix, '-id', $env:ZANT_STORE_PRODUCT_ID) '提交到 Microsoft Store'
        Ok '已提交：在 Partner Center 里查看审核进度'
    }
}

function Folder {
    $zip = Join-Path $env:ZANT_OUT "$env:ZANT_TITLE-$env:ZANT_VERSION-windows.zip"
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Step '压缩程序目录'
    Run 'tar.exe' @('-a', '-c', '-f', $zip, '-C', (Split-Path $env:ZANT_DIR), (Split-Path $env:ZANT_DIR -Leaf)) '压缩'
    Ok "已生成 $zip"
}

switch ($Action) {
    'list-certs' {
        Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
            Where-Object { $_.HasPrivateKey -and $_.NotAfter -gt (Get-Date) } |
            ForEach-Object { $_.Thumbprint + '|' + $_.Subject + '|' + $_.NotAfter.ToString('yyyy-MM-dd') }
    }
    'find-iscc' { Find-Iscc }
    'install-iscc' {
        Run 'winget' @('install', '--id', 'JRSoftware.InnoSetup', '-e', '--accept-package-agreements', '--accept-source-agreements') '安装 Inno Setup'
    }
    'package' {
        New-Item -ItemType Directory -Force -Path $env:ZANT_OUT | Out-Null
        if (Signing) {
            Step '签名程序文件'
            $own = @("$env:ZANT_TITLE.exe", 'zant_skia.dll', 'pdfium.dll') | ForEach-Object { Join-Path $env:ZANT_DIR $_ } | Where-Object { Test-Path $_ }
            Sign-Files $own
            Ok '程序文件已签名'
        }
        switch ($env:ZANT_KIND) {
            'installer' { Installer }
            'store' { Msix }
            'folder' { Folder }
            default { Die "未知的发布方式：$env:ZANT_KIND" }
        }
    }
    default { Die "用法: windows.ps1 list-certs | find-iscc | install-iscc | package" }
}
