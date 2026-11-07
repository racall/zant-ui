# zant.pdf.view on Windows (PDFium): for every PDF in a folder, probe the
# page count, text, links and outline, then open the first and middle page
# in the zant-pdf example and snapshot them; finally drive one viewer with
# the mouse and keyboard (drag selection + Ctrl+C, double click, zoom,
# search, page buttons). Run in the
# logged-in desktop session:
#   powershell -File scripts/win/run-desktop.ps1 -Exe powershell.exe `
#     -Arguments "-NoProfile -ExecutionPolicy Bypass -File <repo>\scripts\win\pdf-check.ps1 -Folder <pdf folder>" -Wait 1200
param(
    [Parameter(Mandatory = $true)][string]$Folder,
    [string]$App = "",
    [string]$Out = ""
)
# defaults here: $PSScriptRoot is empty inside param() on Windows PowerShell 5.1
if (!$App) { $App = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "zig-out\bin\zant-pdf.exe") }
if (!$Out) { $Out = (Join-Path $env:TEMP "zant-checks\pdf") }
$ErrorActionPreference = "Stop"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Out
New-Item -ItemType Directory -Force $Out | Out-Null
$report = Join-Path $Out "report.txt"

function Run([string]$file, [hashtable]$envs, [int]$timeout) {
    $psi = New-Object Diagnostics.ProcessStartInfo $App
    $psi.Arguments = "`"$file`""
    $psi.UseShellExecute = $false
    $psi.RedirectStandardError = $true
    $psi.RedirectStandardOutput = $true
    foreach ($k in $envs.Keys) { $psi.EnvironmentVariables[$k] = $envs[$k] }
    $p = [Diagnostics.Process]::Start($psi)
    $err = $p.StandardError.ReadToEndAsync()
    $std = $p.StandardOutput.ReadToEndAsync()
    $done = $p.WaitForExit($timeout)
    if (!$done) { $p.Kill(); $p.WaitForExit() }
    [pscustomobject]@{ Done = $done; Code = $p.ExitCode; Text = $std.Result + $err.Result }
}

$i = 0
# "._*" are macOS AppleDouble files, not PDFs
foreach ($file in (Get-ChildItem $Folder -Filter *.pdf | Where-Object { $_.Name -notlike "._*" } | Sort-Object Name)) {
    $i++
    $probe = Run $file.FullName @{ ZANT_PDF_PROBE = "1" } 60000
    $pages = 0; $t0 = 0; $links = 0
    if ($probe.Text -match "pages=(\d+) .*text0=(\d+) links=(\d+)") { $pages = [int]$Matches[1]; $t0 = [int]$Matches[2]; $links = [int]$Matches[3] }
    $problems = @()
    if (!$probe.Done) { $problems += "probe timeout" }
    elseif ($probe.Code -ne 0) { $problems += "probe exit $($probe.Code)" }
    if ($pages -eq 0) { $problems += "no pages" }
    $mid = [math]::Floor($pages / 2)
    foreach ($view in @(@{ Name = "p1"; Page = "0" }, @{ Name = "mid"; Page = "$mid" })) {
        if ($view.Name -eq "mid" -and $pages -lt 2) { continue }
        $png = Join-Path $Out ("{0:D2}-{1}.png" -f $i, $view.Name)
        $r = Run $file.FullName @{ ZANT_SNAPSHOT = $png; ZANT_SNAPSHOT_EXIT = "1"; ZANT_PDF_PAGE = $view.Page } 60000
        if (!$r.Done) { $problems += "$($view.Name): timeout" }
        elseif ($r.Code -ne 0) { $problems += "$($view.Name): exit $($r.Code)" }
        if (!(Test-Path $png)) { $problems += "$($view.Name): no snapshot" }
        if ($r.Text -match "panic|error\(|leaked") { $problems += "$($view.Name): " + (($r.Text -split "`n" | Where-Object { $_ -match "panic|error\(|leaked" } | Select-Object -First 1).Trim()) }
    }
    $status = if ($problems.Count -eq 0) { "PASS" } else { "FAIL" }
    Add-Content -Encoding UTF8 $report ("{0} {1:D2} pages={2} text_p1={3} links={4} {5} {6}" -f $status, $i, $pages, $t0, $links, $file.Name, ($problems -join "; "))
}
# interactive: drag selection + Ctrl+C, double click, zoom, search, pages
. (Join-Path $PSScriptRoot "ui-auto.ps1")
$script:ResultFile = $report
# report script errors and keep going (one broken step must not hide the rest)
trap { Add-Content -Encoding UTF8 $report ("ERROR $_ at $($_.InvocationInfo.ScriptLineNumber): $($_.InvocationInfo.Line.Trim())"); continue }
# the first PDF with a text layer on page 1 and a few pages
$sample = Get-ChildItem $Folder -Filter *.pdf | Where-Object { $_.Name -notlike "._*" } | Sort-Object Length | Where-Object {
    (Run $_.FullName @{ ZANT_PDF_PROBE = "1" } 60000).Text -match "pages=([3-9]|\d\d+) .*text0=[1-9]\d\d"
} | Select-Object -First 1
if ($sample) {
    Set-Clipboard -Value "before"
    $viewer = Start-ZantApp $App $Out -Arguments "`"$($sample.FullName)`""
    $page = Find-Key $viewer "pdf-p0"
    Write-Result ($page -ne $null) "interactive: page 1 laid out ($($sample.Name))"
    if ($page) {
        # drag across the upper body of page 1, then copy
        Drag-From-To $viewer ($page.X + $page.W * 0.12) ($page.Y + $page.H * 0.2) ($page.X + $page.W * 0.88) ($page.Y + $page.H * 0.4)
        Shot $viewer "interactive-selection"
        Send-Keys "^c"
        $clip = Get-Clipboard -Raw
        Write-Result ($clip -ne $null -and $clip -ne "before" -and $clip.Length -gt 10) "interactive: drag + Ctrl+C copies the selection ($(if ($clip) { $clip.Length } else { 0 }) chars)"
        Set-Clipboard -Value "before"
        Double-Click-At $viewer ($page.X + $page.W * 0.5) ($page.Y + $page.H * 0.3)
        Send-Keys "^c"
        $word = Get-Clipboard -Raw
        Write-Result ($word -ne $null -and $word -ne "before" -and $word.Length -lt 40) "interactive: double click selects a word ('$word')"
        Send-Keys "{ESC}"
    }
    $before = Find-Key $viewer "pdf-p0"
    Click-Key $viewer "pdf-zin" | Out-Null
    Click-Key $viewer "pdf-zin" | Out-Null
    $after = Find-Key $viewer "pdf-p0"
    Write-Result ($before -and $after -and $after.W -gt $before.W * 1.1) "interactive: zoom in enlarges the page"
    Shot $viewer "interactive-zoom"
    Click-Key $viewer "pdf-zoom" | Out-Null
    $vp = Find-Key $viewer "pdf-vp"
    if ($vp) {
        Wheel-At $viewer ($vp.X + $vp.W / 2) ($vp.Y + $vp.H / 2) -3
        Shot $viewer "interactive-scrolled"
    }
    Click-Key $viewer "pdf-search" | Out-Null
    Send-Keys "the"
    Start-Sleep -Milliseconds 800
    Shot $viewer "interactive-search"
    Send-Keys "{ENTER}"
    Shot $viewer "interactive-search-next"
    Click-Key $viewer "pdf-next" | Out-Null
    Click-Key $viewer "pdf-next" | Out-Null
    Shot $viewer "interactive-page3"
    Write-Result (!$viewer.Process.HasExited) "interactive: the viewer keeps running"
    $errors = Get-Content -ErrorAction SilentlyContinue $viewer.Stderr | Where-Object { $_ -match "panic|error\(|leaked" }
    Write-Result ($errors.Count -eq 0) "interactive: no panics or leaks in stderr"
    Stop-ZantApp $viewer
}
Add-Content -Encoding UTF8 $report "done"
