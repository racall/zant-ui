# Hot reload check with real mouse and keyboard on the Windows desktop.
# Run it in the logged-in session (scripts/win/run-desktop.ps1 -Exe pwsh.exe
# -Arguments "-File scripts\win\hot-check.ps1 -Work <dir>"). It creates an app
# with `zant create`, starts `zant run`, clicks and types for real, edits the
# sources and checks that the same window refreshes with the state kept.
param(
    [string]$Zant = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "target\debug\zant.exe"),
    [Parameter(Mandatory = $true)][string]$Work
)
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\ui-auto.ps1"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Work
New-Item -ItemType Directory -Force $Work | Out-Null
$out = Join-Path $Work "evidence"; New-Item -ItemType Directory -Force $out | Out-Null
$script:ResultFile = Join-Path $out "result.txt"
$shots = Join-Path $out "shots"

Push-Location $Work
& $Zant create hotapp --yes *> (Join-Path $out "create.log")
Pop-Location
$app = Join-Path $Work "hotapp"
$appTsx = Join-Path $app "src\App.tsx"
$card = Join-Path $app "src\components\CounterCard.tsx"
function Read-Text($p) { [IO.File]::ReadAllText($p) }
function Write-Text($p, $t) { [IO.File]::WriteAllText($p, $t, (New-Object Text.UTF8Encoding $false)) }
$t = Read-Text $appTsx
$t = $t.Replace("<Input value=", '<Input key="name" value=').Replace("<Text color=", '<Text key="count" color=')
$rows = (0..29 | ForEach-Object { "<Paragraph>Scrollable row $_</Paragraph>" }) -join ""
$t = $t.Replace("    </View>", "      <Column key=`"scroller`" height={100} overflow=`"scroll`">$rows</Column>`n    </View>").Replace("Text }", "Text, Column }")
Write-Text $appTsx $t
Write-Text $card ((Read-Text $card).Replace('<Button variant="primary"', '<Button key="inc" variant="primary"'))

$log = Join-Path $out "run.log"
$env:ZANT_HOT_SNAPSHOT = $shots
$env:ZANT_HOT_TRACE = "1"
$env:ZANT_FRAME_STATS = "1"
$env:ZANT_NO_MOTION = "1"
$env:ZANT_LAYOUT_FILE = Join-Path $out "layout.txt"
$run = Start-Process -FilePath $Zant -ArgumentList "run" -WorkingDirectory $app -PassThru `
    -RedirectStandardError $log -RedirectStandardOutput (Join-Path $out "run.out")

function Reloads { @(Get-ChildItem -ErrorAction SilentlyContinue $shots -Filter "reload-*.png").Count }
function Wait-For([scriptblock]$cond, [string]$what, [int]$seconds = 180) {
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) { if (& $cond) { return $true }; Start-Sleep -Milliseconds 100 }
    Write-Result $false "timeout: $what"; return $false
}
function Host-Process { Get-Process zant-hot-host -ErrorAction SilentlyContinue | Select-Object -First 1 }
function Latest-State {
    $f = Get-ChildItem -ErrorAction SilentlyContinue $shots -Filter "state-*.json" | Sort-Object LastWriteTime | Select-Object -Last 1
    if ($f) { Get-Content -Raw $f.FullName | ConvertFrom-Json } else { $null }
}
function Edit-And-Wait([string]$path, [scriptblock]$change, [string]$name) {
    $n = Reloads; $start = Get-Date
    Write-Text $path (& $change (Read-Text $path))
    $ok = Wait-For { (Reloads) -gt $n } $name
    $ms = [int]((Get-Date) - $start).TotalMilliseconds
    Add-Content $script:ResultFile "TIME $name save-to-refresh=${ms}ms"
    Start-Sleep -Milliseconds 400
    return $ok
}

if (!(Wait-For { (Reloads) -gt 0 } "first frame" 600)) { exit 1 }
$hostProc = Host-Process
$ui = [pscustomobject]@{ Process = $hostProc; Hwnd = $hostProc.MainWindowHandle; Layout = $env:ZANT_LAYOUT_FILE; Stderr = $log; Out = $out; Scale = [ZantAuto]::GetDpiForWindow($hostProc.MainWindowHandle) / 96.0; Seen = 0 }
[ZantAuto]::SetForegroundWindow($ui.Hwnd) | Out-Null
Start-Sleep -Milliseconds 500

# real input: three clicks, typing, scrolling
for ($i = 0; $i -lt 3; $i++) { Click-Key $ui "inc" | Out-Null }
Click-Key $ui "name" | Out-Null
Send-Keys "{END}abc"
$s = Find-Key $ui "scroller"; Wheel-At $ui ($s.X + $s.W / 2) ($s.Y + $s.H / 2) -3
Shot $ui "1-before"

$hostPid = $hostProc.Id
Edit-And-Wait $appTsx { param($x) $x.Replace("Scrollable row 0<", "Reloaded row 0<") } "tsx-text" | Out-Null
Shot $ui "2-after-tsx"
Write-Result ((Host-Process).Id -eq $hostPid) "same host process after a TSX edit (pid $hostPid)"
Edit-And-Wait $appTsx { param($x) $x + "`n" } "state-save" | Out-Null
$st = Latest-State
Write-Result ($st.app.clicks -eq 3) "clicks kept (got $($st.app.clicks))"
Write-Result ($st.app.name -like "*abc") "typed text kept (got '$($st.app.name)')"
Write-Result ($st.view.nodes.name.focus -eq $true) "input focus kept"
Write-Result ($st.view.nodes.scroller.scroll -gt 0) "scroll kept (got $($st.view.nodes.scroller.scroll))"

# a build error shows the panel over the running UI, the fix clears it
$good = Read-Text $appTsx
$errs = (Select-String -Path $log -Pattern "error" -SimpleMatch -ErrorAction SilentlyContinue).Count
Write-Text $appTsx ($good + "`nexport default broken !!!`n")
Wait-For { (Select-String -Path $log -Pattern "error" -SimpleMatch).Count -gt $errs } "TSX error reported" | Out-Null
Start-Sleep -Seconds 1
Shot $ui "3-error-panel"
Write-Result ((Host-Process).Id -eq $hostPid) "window stays on a build error"
Edit-And-Wait $appTsx { param($x) $good } "tsx-fixed" | Out-Null
Shot $ui "4-fixed"

$native = Join-Path $app "native\system.rs"
Edit-And-Wait $native { param($x) $x + "`n// edited`n" } "native-rs" | Out-Null
Write-Result ((Host-Process).Id -eq $hostPid) "native/*.rs reloads in place"

# config change: zant run restarts the app
$cfg = Join-Path $app "app.config.json"
Write-Text $cfg ((Read-Text $cfg).Replace('"width": 680', '"width": 700'))
Wait-For { $h = Host-Process; $h -and $h.Id -ne $hostPid } "restart on app.config.json" | Out-Null
Write-Result ((Host-Process).Id -ne $hostPid) "app.config.json restarts the app"

Get-Process zant-hot-host -ErrorAction SilentlyContinue | ForEach-Object { $_.CloseMainWindow() | Out-Null }
$exited = Wait-For { $run.HasExited } "zant run ends with the window" 20
Write-Result $exited "closing the window ends zant run"
if (!$run.HasExited) { Stop-Process -Id $run.Id -Force }
Get-Process zant-hot-host -ErrorAction SilentlyContinue | Stop-Process -Force
Add-Content $script:ResultFile "DONE"
