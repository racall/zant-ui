# Live-resize check on the Windows desktop: drags the bottom-right corner of
# a Zant UI window with the real mouse (the system's sizing loop), takes
# screenshots during the drag, and reads ZANT_FRAME_STATS to report the frame
# rate while sizing. Then checks the layout at the minimum size. Run in the
# logged-in session:
#   powershell -File scripts/win/run-desktop.ps1 -Exe powershell.exe `
#     -Arguments "-NoProfile -ExecutionPolicy Bypass -File <repo>\scripts\win\resize-check.ps1" -Wait 300
param(
    [string]$Exe = "",
    [string]$Arguments = "",
    [string]$Out = "",
    [string]$WorkDir = ""
)
# defaults here: $PSScriptRoot is empty inside param() on Windows PowerShell 5.1
if (!$Exe) { $Exe = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "zig-out\bin\zant-components.exe") }
if (!$Out) { $Out = (Join-Path $env:TEMP "zant-checks\resize") }
if (!$WorkDir) { $WorkDir = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) }
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "ui-auto.ps1")
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ZantResize {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int hh, bool repaint);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("winmm.dll")] public static extern uint timeBeginPeriod(uint p);
  // moves the cursor through (dx, dy) steps at `hz`, timed with a stopwatch
  public static int Drag(int x0, int y0, int[] dx, int[] dy, int hz) {
    timeBeginPeriod(1);
    var sw = System.Diagnostics.Stopwatch.StartNew();
    double step = 1000.0 / hz;
    for (int i = 0; i < dx.Length; i++) {
      SetCursorPos(x0 + dx[i], y0 + dy[i]);
      double due = (i + 1) * step;
      while (sw.Elapsed.TotalMilliseconds < due) { if (due - sw.Elapsed.TotalMilliseconds > 2) System.Threading.Thread.Sleep(1); }
    }
    return (int)sw.ElapsedMilliseconds;
  }
}
"@
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Out
New-Item -ItemType Directory -Force $Out | Out-Null
$script:ResultFile = Join-Path $Out "report.txt"
Set-Location $WorkDir

# animation frame rate first: the example animates by itself for 9 s
if ($Exe -like "*zant-components*") {
    # "quiet": no [event] lines; writing to stderr costs ~10 ms per line on Windows
    $auto = Start-ZantApp $Exe $Out -Env @{ ZANT_FRAME_STATS = "1"; ZANT_AUTOPLAY = "quiet" } -Motion
    Start-Sleep -Seconds 9
    $lines = @(Get-Content $auto.Stderr | Where-Object { $_ -like "``[frames``]*" } | Select-Object -Skip 1)
    Add-Content -Encoding UTF8 $script:ResultFile "autoplay (animations, no input):"
    foreach ($line in $lines) { Add-Content -Encoding UTF8 $script:ResultFile ("  " + $line) }
    $worst = @($lines | ForEach-Object { if ($_ -match "max_frame_ms=([\d.]+)") { [double]$Matches[1] } })
    $slowest = if ($worst.Count) { ($worst | Measure-Object -Maximum).Maximum } else { 99 }
    Write-Result ($slowest -lt 16.7) ("slowest animation frame (Zig + Skia recording): {0:N1} ms" -f $slowest)
    Stop-ZantApp $auto
    $env:ZANT_AUTOPLAY = ""
}
$app = Start-ZantApp $Exe $Out -Arguments $Arguments -Env @{ ZANT_FRAME_STATS = "1" } -Motion
Start-Sleep -Milliseconds 1200
$r = New-Object ZantResize+RECT
[ZantResize]::GetWindowRect($app.Hwnd, [ref]$r) | Out-Null
Shot $app "00-start"

function Frames {
    @(Get-Content -ErrorAction SilentlyContinue $app.Stderr | Where-Object { $_ -like "``[frames``]*" })
}
$before = (Frames).Count

# grab the bottom-right corner (just inside the resize border) and drag:
# grow, shrink below the minimum, grow again, each step a real mouse move
$x0 = $r.R - 4; $y0 = $r.B - 4
[ZantAuto]::SetCursorPos($x0, $y0) | Out-Null
Start-Sleep -Milliseconds 300
[ZantAuto]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 200
# 120 cursor moves per second: grow (2 s), shrink below the minimum (2 s),
# grow again (2 s); screenshots between the phases while the button is held
$dx = New-Object System.Collections.Generic.List[int]; $dy = New-Object System.Collections.Generic.List[int]
function Segment($fromX, $fromY, $toX, $toY, $steps) {
    for ($i = 1; $i -le $steps; $i++) { $dx.Add([int]($fromX + ($toX - $fromX) * $i / $steps)); $dy.Add([int]($fromY + ($toY - $fromY) * $i / $steps)) }
}
$hz = 120
$dragMs = 0
$moves = 0
foreach ($seg in @(@(0, 0, 360, 240, "01-growing"), @(360, 240, -840, -600, "02-shrinking"), @(-840, -600, -100, -40, "03-growing-again"))) {
    $dx.Clear(); $dy.Clear()
    Segment $seg[0] $seg[1] $seg[2] $seg[3] (2 * $hz)
    $dragMs += [ZantResize]::Drag($x0, $y0, $dx.ToArray(), $dy.ToArray(), $hz)
    $moves += $dx.Count
    Shot $app $seg[4]
}
$path = 1..$moves
[ZantAuto]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 1500

$during = @(Frames | Select-Object -Skip $before)
Add-Content -Encoding UTF8 $script:ResultFile ("drag: {0} mouse moves in {1:N0} ms ({2} Hz)" -f $moves, $dragMs, $hz)
foreach ($line in $during) { Add-Content -Encoding UTF8 $script:ResultFile ("  " + $line) }
# frames drawn at a new size per second, and resize -> frame latency
# seconds spent actually resizing (not clamped at the minimum, not pausing
# for a screenshot): at least half a second's worth of new sizes
$resizeFps = @($during | ForEach-Object { if ($_ -match "resize_frames=(\d+)") { [int]$Matches[1] } } | Where-Object { $_ -ge 30 })
$latency = @($during | ForEach-Object { if ($_ -match "max_resize_latency_ms=([\d.]+)") { [double]$Matches[1] } })
$worst = @($during | ForEach-Object { if ($_ -match "max_frame_ms=([\d.]+)") { [double]$Matches[1] } })
$avg = if ($resizeFps.Count) { ($resizeFps | Measure-Object -Average).Average } else { 0 }
$slowest = if ($worst.Count) { ($worst | Measure-Object -Maximum).Maximum } else { 0 }
$lat = if ($latency.Count) { ($latency | Measure-Object -Maximum).Maximum } else { 0 }
Write-Result ($avg -ge 55) ("frames drawn at new sizes while dragging: avg {0:N1} per second (60 Hz display)" -f $avg)
Write-Result ($lat -lt 20) ("longest delay from a size change to its frame: {0:N1} ms" -f $lat)
Write-Result ($slowest -lt 16.7) ("slowest frame (Zig + Skia recording) while sizing: {0:N1} ms" -f $slowest)
Write-Result (!$app.Process.HasExited) "app survives live resize"

# layout after the drag: every keyed node inside the window
Shot $app "04-after-drag"
$c = New-Object ZantAuto+RECT; [ZantAuto]::GetClientRect($app.Hwnd, [ref]$c) | Out-Null
$wl = $c.R / $app.Scale; $hl = $c.B / $app.Scale
$outside = @(Get-Content $app.Layout | Where-Object {
    $f = $_ -split "`t"; $x = [double]$f[1]; $w = [double]$f[3]
    $f[0] -notlike "zant-caption*" -and ($x -lt -0.5 -or $x + $w -gt $wl + 0.5)
})
Write-Result ($outside.Count -eq 0) ("layout fits the window width ({0:N0}x{1:N0}){2}" -f $wl, $hl, $(if ($outside.Count) { ": " + ($outside[0..2] -join " | ") } else { "" }))

# the minimum size is enforced (dragging below it stopped there)
[ZantResize]::GetWindowRect($app.Hwnd, [ref]$r) | Out-Null
[ZantResize]::MoveWindow($app.Hwnd, $r.L, $r.T, 200, 200, $true) | Out-Null
Start-Sleep -Milliseconds 800
[ZantAuto]::GetClientRect($app.Hwnd, [ref]$c) | Out-Null
Write-Result ($c.R -gt 200 -and $c.B -gt 200) ("minimum window size enforced: client {0}x{1} px" -f $c.R, $c.B)
Shot $app "05-minimum"
Stop-ZantApp $app
Add-Content -Encoding UTF8 $script:ResultFile "done"
