# Scaffold check in a real console window (OEM code page 936 like a Chinese
# Windows default): runs `zant create`, types a Chinese title, a window style
# and an app identifier (an invalid one first) with Unicode key events,
# screenshots the console, and checks the generated app.config.json has the
# shared settings only. Run in the logged-in
# session:
#   powershell -File scripts/win/run-desktop.ps1 -Exe powershell.exe `
#     -Arguments "-NoProfile -ExecutionPolicy Bypass -File <repo>\scripts\win\scaffold-check.ps1" -Wait 120
param(
    [string]$Zant = "",
    [string]$Out = ""
)
if (!$Zant) { $Zant = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "zig-out\bin\zant.exe") }
if (!$Out) { $Out = (Join-Path $env:TEMP "zant-checks\scaffold") }
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ZantKeys {
  [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort vk; public ushort scan; public uint flags; public uint time; public IntPtr extra; }
  [StructLayout(LayoutKind.Explicit, Size = 40)] public struct INPUT { [FieldOffset(0)] public uint type; [FieldOffset(8)] public KEYBDINPUT ki; }
  [DllImport("user32.dll")] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  public delegate bool EnumProc(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr p);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  // the visible top-level window whose title contains `part` (conhost or Windows Terminal)
  public static IntPtr FindTitled(string part) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, p) => {
      if (!IsWindowVisible(h)) return true;
      var sb = new System.Text.StringBuilder(512); GetWindowText(h, sb, 512);
      if (sb.ToString().Contains(part)) { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  // types text as Unicode key events (any script, no input method needed)
  public static void Type(string text) {
    foreach (char c in text) {
      var down = new INPUT { type = 1 }; down.ki.scan = c; down.ki.flags = 0x4;
      var up = new INPUT { type = 1 }; up.ki.scan = c; up.ki.flags = 0x4 | 0x2;
      SendInput(2, new[] { down, up }, Marshal.SizeOf(typeof(INPUT)));
      System.Threading.Thread.Sleep(20);
    }
  }
  public static void Enter() {
    var down = new INPUT { type = 1 }; down.ki.vk = 0x0D;
    var up = new INPUT { type = 1 }; up.ki.vk = 0x0D; up.ki.flags = 0x2;
    SendInput(2, new[] { down, up }, Marshal.SizeOf(typeof(INPUT)));
  }
}
"@
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Out
New-Item -ItemType Directory -Force $Out | Out-Null
$report = Join-Path $Out "report.txt"
function Result([bool]$ok, [string]$name) { Add-Content -Encoding UTF8 $report ("{0} {1}" -f $(if ($ok) { "PASS" } else { "FAIL" }), $name) }
function Shot($h, [string]$name) {
    Start-Sleep -Milliseconds 400
    $r = New-Object ZantKeys+RECT; [ZantKeys]::GetWindowRect($h, [ref]$r) | Out-Null
    $bmp = New-Object Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
    $g = [Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
    $bmp.Save((Join-Path $Out "$name.png")); $g.Dispose(); $bmp.Dispose()
}

$work = Join-Path $Out "work"
New-Item -ItemType Directory -Force $work | Out-Null
$title = "剪贴板测试"
$bundle = "com.xx.test"
$p = Start-Process cmd.exe -ArgumentList "/k title zant-scaffold-check & chcp 936 >nul & cd /d `"$work`" & `"$Zant`" create demo-app" -WindowStyle Normal -PassThru
# a new console window (conhost, or Windows Terminal when it is the default
# terminal, whose window belongs to another process): found by its title
$h = [IntPtr]::Zero
for ($i = 0; $i -lt 50 -and $h -eq [IntPtr]::Zero; $i++) {
    Start-Sleep -Milliseconds 200; $p.Refresh(); $h = $p.MainWindowHandle
    if ($h -eq [IntPtr]::Zero -and $i -gt 5) { $h = [ZantKeys]::FindTitled("zant-scaffold-check") }
}
Add-Content -Encoding UTF8 $report ("console window: {0}" -f $h)
Start-Sleep -Seconds 2
[ZantKeys]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 300
Shot $h "01-prompt"
[ZantKeys]::Type($title); Start-Sleep -Milliseconds 300
Shot $h "02-typed"
[ZantKeys]::Enter()
# window style: hidden title bar
Start-Sleep -Milliseconds 800; [ZantKeys]::Type("2"); [ZantKeys]::Enter()
# app identifier: an invalid one first (asked again), then a valid one
Start-Sleep -Milliseconds 800; [ZantKeys]::Type("bad"); [ZantKeys]::Enter()
Start-Sleep -Milliseconds 800; [ZantKeys]::Type($bundle); [ZantKeys]::Enter()
$config = Join-Path $work "demo-app\app.config.json"
for ($i = 0; $i -lt 60 -and !(Test-Path $config); $i++) { Start-Sleep -Milliseconds 500 }
Start-Sleep -Seconds 2
Shot $h "03-done"
$cfg = if (Test-Path $config) { Get-Content -Raw -Encoding UTF8 $config | ConvertFrom-Json } else { $null }
Result ($cfg -and $cfg.title -eq $title) ("app.config.json title: '{0}'" -f $cfg.title)
Result ($cfg -and $cfg.bundle_id -eq $bundle) ("app.config.json bundle_id: '{0}'" -f $cfg.bundle_id)
Result ($cfg -and $cfg.window.style -eq "hidden-titlebar") ("window style: '{0}'" -f $cfg.window.style)
# only shared settings on Windows: no macOS section or bundle files
Result ($cfg -and !$cfg.macos -and !(Test-Path (Join-Path $work "demo-app\Info.plist"))) "no macOS-only settings or files"
& cmd.exe /c "chcp" | Out-File -Encoding UTF8 (Join-Path $Out "chcp.txt")
Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
Get-Process cmd -ErrorAction SilentlyContinue | Where-Object { $_.Parent.Id -eq $p.Id } | Stop-Process -Force -ErrorAction SilentlyContinue
Add-Content -Encoding UTF8 $report "done"
