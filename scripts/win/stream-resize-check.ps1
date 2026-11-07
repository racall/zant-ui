# Streaming + window interaction check on the Windows desktop: replays a
# recorded SSE reply in zant-md-stream and, while it streams, drags the window
# by its title bar and resizes it from the bottom-right corner with the real
# mouse. Screenshots every phase and reports ZANT_FRAME_STATS. Run in the
# logged-in session:
#   powershell -File scripts/win/run-desktop.ps1 -Exe powershell.exe `
#     -Arguments "-NoProfile -ExecutionPolicy Bypass -File <repo>\scripts\win\stream-resize-check.ps1 -Sse <file>" -Wait 300
param(
    [Parameter(Mandatory = $true)][string]$Sse,
    [string]$Exe = "",
    [string]$Out = "",
    [string]$WorkDir = "",
    [int]$History = 30
)
if (!$WorkDir) { $WorkDir = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) }
if (!$Exe) { $Exe = (Join-Path $WorkDir "zig-out\bin\zant-md-stream.exe") }
if (!$Out) { $Out = (Join-Path $env:TEMP "zant-checks\stream-resize") }
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "ui-auto.ps1")
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ZantDrag {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, IntPtr e);
  [DllImport("winmm.dll")] public static extern uint timeBeginPeriod(uint p);
  // presses at (x0, y0), moves straight to (x1, y1) in steps moves at 120 Hz;
  // release: let go at the end
  public static void Drag(int x0, int y0, int x1, int y1, int steps, bool press, bool release) {
    timeBeginPeriod(1);
    if (press) { SetCursorPos(x0, y0); System.Threading.Thread.Sleep(200); mouse_event(0x2, 0, 0, 0, IntPtr.Zero); System.Threading.Thread.Sleep(150); }
    var sw = System.Diagnostics.Stopwatch.StartNew();
    for (int i = 1; i <= steps; i++) {
      SetCursorPos(x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps);
      double due = i * (1000.0 / 120);
      while (sw.Elapsed.TotalMilliseconds < due) { if (due - sw.Elapsed.TotalMilliseconds > 2) System.Threading.Thread.Sleep(1); }
    }
    if (release) { System.Threading.Thread.Sleep(100); mouse_event(0x4, 0, 0, 0, IntPtr.Zero); }
  }
}
"@
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Out
New-Item -ItemType Directory -Force $Out | Out-Null
$script:ResultFile = Join-Path $Out "report.txt"
Set-Location $WorkDir

$app = Start-ZantApp $Exe $Out -Arguments "--sse-replay `"$Sse`"" -Env @{ ZANT_FRAME_STATS = "1"; ZANT_STREAM_HISTORY = "$History"; ZANT_STREAM_MS = "25" } -Motion
Start-Sleep -Seconds 3
Shot $app "01-streaming"

# move: hold the title bar and drag the window around (twice, the second
# time with a screenshot while the button is still down)
$r = New-Object ZantDrag+RECT
[ZantDrag]::GetWindowRect($app.Hwnd, [ref]$r) | Out-Null
$startL = $r.L; $startT = $r.T
$tx = [int]($r.L + ($r.R - $r.L) * 0.4); $ty = $r.T + [int](16 * $app.Scale)
[ZantDrag]::Drag($tx, $ty, $tx + 300, $ty + 120, 120, $true, $false)
Shot $app "02-moving"
[ZantDrag]::Drag($tx + 300, $ty + 120, $tx - 100, $ty + 40, 120, $false, $true)
Start-Sleep -Milliseconds 400
[ZantDrag]::GetWindowRect($app.Hwnd, [ref]$r) | Out-Null
Write-Result ([Math]::Abs($r.L - $startL + 100) -le 2 -and [Math]::Abs($r.T - $startT - 40) -le 2) ("window moved by title bar: ({0},{1}) -> ({2},{3})" -f $startL, $startT, $r.L, $r.T)
Shot $app "03-moved"

# resize: grow, shrink to narrow, grow back, screenshots while dragging
$x0 = $r.R - 4; $y0 = $r.B - 4
[ZantDrag]::Drag($x0, $y0, $x0 + 300, $y0 + 150, 240, $true, $false)
Shot $app "04-resize-wide"
[ZantDrag]::Drag($x0 + 300, $y0 + 150, $x0 - 450, $y0 - 250, 240, $false, $false)
Shot $app "05-resize-narrow"
[ZantDrag]::Drag($x0 - 450, $y0 - 250, $x0 + 100, $y0 + 50, 240, $false, $true)
Start-Sleep -Milliseconds 600
Shot $app "06-resized"

# rest of the stream, then the end of the reply
for ($i = 0; $i -lt 240 -and !(Select-String -Quiet -Path $app.Stderr -SimpleMatch -Pattern "[stream] done" -ErrorAction SilentlyContinue); $i++) { Start-Sleep -Milliseconds 500 }
Start-Sleep -Seconds 2
Shot $app "07-done"

$frames = @(Get-Content $app.Stderr | Where-Object { $_ -like "``[frames``]*" })
foreach ($line in $frames) { Add-Content -Encoding UTF8 $script:ResultFile ("  " + $line) }
$worst = @($frames | Select-Object -Skip 1 | ForEach-Object { if ($_ -match "max_frame_ms=([\d.]+)") { [double]$Matches[1] } })
$slowest = if ($worst.Count) { ($worst | Measure-Object -Maximum).Maximum } else { 99 }
Write-Result ($slowest -lt 16.7) ("slowest frame after the first second (streaming, moving, resizing): {0:N1} ms" -f $slowest)
$slow = @(Get-Content $app.Stderr | Where-Object { $_ -like "*slow-frame*" })
Add-Content -Encoding UTF8 $script:ResultFile ("slow frames: {0}" -f $slow.Count)
foreach ($line in ($slow | Select-Object -First 10)) { Add-Content -Encoding UTF8 $script:ResultFile ("  " + $line) }
Write-Result (!$app.Process.HasExited) "app survives streaming + move + resize"
Stop-ZantApp $app
Add-Content -Encoding UTF8 $script:ResultFile "done"
