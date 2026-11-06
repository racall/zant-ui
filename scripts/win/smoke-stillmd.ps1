# Interactive smoke test of the stillmd example on the Windows desktop:
# typing (incl. non-ASCII), save with CRLF kept, find, dark mode, clipboard,
# caption buttons (maximize / restore), close confirmation. Screenshots and
# a result log go to %TEMP%\zant-checks\smoke. Must run in the logged-in session:
#   powershell -File scripts/win/run-desktop.ps1 -Exe powershell.exe `
#     -Arguments "-NoProfile -ExecutionPolicy Bypass -File <repo>\scripts\win\smoke-stillmd.ps1 -App <stillmd>\zig-out\bin\app.exe" -Wait 120
param([Parameter(Mandatory = $true)][string]$App)
$ErrorActionPreference = "Stop"
# a failed check must not stop the remaining ones
function Read-Text($path) { if (Test-Path $path) { [IO.File]::ReadAllText($path) } else { "" } }
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr LoadKeyboardLayout(string id, uint flags);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@
[W]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null

$out = Join-Path $env:TEMP "zant-checks\smoke"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $out
New-Item -ItemType Directory -Force $out | Out-Null
$log = Join-Path $out "result.txt"
function Log($m) { Add-Content -Encoding UTF8 $log $m }
function Check($name, $ok) { Log ("{0} {1}" -f ($(if ($ok) { "PASS" } else { "FAIL" }), $name)) }

# a CRLF document
$doc = Join-Path $out "doc.md"
[IO.File]::WriteAllText($doc, "# Smoke`r`n`r`nfirst line`r`n", (New-Object Text.UTF8Encoding $false))

$env:ZANT_NO_MOTION = "1"
# start in the editor whatever view the last session saved (not persisted)
$env:STILL_MODE = "edit"
$p = Start-Process -FilePath $App -ArgumentList "`"$doc`"" -PassThru -RedirectStandardError (Join-Path $out "app-stderr.txt") -RedirectStandardOutput (Join-Path $out "app-stdout.txt")
$h = [IntPtr]::Zero
for ($i = 0; $i -lt 50 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 200; $p.Refresh(); $h = $p.MainWindowHandle }
if ($h -eq [IntPtr]::Zero) { Log "FAIL window did not appear"; exit 1 }
Start-Sleep -Milliseconds 800
[W]::SetForegroundWindow($h) | Out-Null
$scale = [W]::GetDpiForWindow($h) / 96.0

function Rect { $r = New-Object W+RECT; [W]::GetWindowRect($h, [ref]$r) | Out-Null; $r }
function Shot($name) {
  Start-Sleep -Milliseconds 400
  $r = Rect
  $bmp = New-Object Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
  $g = [Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
  $bmp.Save((Join-Path $out "$name.png"))
  $g.Dispose(); $bmp.Dispose()
}
function ScreenShot($name) {
  Start-Sleep -Milliseconds 500
  $b = [Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bmp = New-Object Drawing.Bitmap $b.Width, $b.Height
  $g = [Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.X, $b.Y, 0, 0, $bmp.Size)
  $bmp.Save((Join-Path $out "$name.png"))
  $g.Dispose(); $bmp.Dispose()
}
# screen point of a client point given in logical px (-FromRight: from the right edge)
function ClientPoint($lx, $ly, [switch]$FromRight) {
  $c = New-Object W+RECT; [W]::GetClientRect($h, [ref]$c) | Out-Null
  $o = New-Object W+POINT; [W]::ClientToScreen($h, [ref]$o) | Out-Null
  $x = if ($FromRight) { $o.X + $c.R - [int]($lx * $scale) } else { $o.X + [int]($lx * $scale) }
  @($x, ($o.Y + [int]($ly * $scale)))
}
function Click($lx, $ly, [switch]$FromRight) {
  $pt = ClientPoint $lx $ly -FromRight:$FromRight
  $x = $pt[0]; $y = $pt[1]
  [W]::SetCursorPos($x, $y) | Out-Null; Start-Sleep -Milliseconds 150
  [W]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 60
  [W]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 250
}
function Keys($k) { [Windows.Forms.SendKeys]::SendWait($k); Start-Sleep -Milliseconds 250 }
function Title { $sb = New-Object Text.StringBuilder 256; [W]::GetWindowText($h, $sb, 256) | Out-Null; $sb.ToString() }

Shot "01-open"
Check "opened CRLF document" ((Title) -like "still*")

# type at the end of the document (non-ASCII through WM_CHAR)
Click 500 400
Keys "^{END}"
Keys "{ENTER}Hello 世界 ä"
Shot "02-typed"
Check "window title marks unsaved changes" ((Title).StartsWith("*"))

# save: CRLF kept, text appended
Keys "^s"
Start-Sleep -Milliseconds 600
$saved = Read-Text $doc
Check "saved text contains the typed line" ($saved.Contains("Hello 世界 ä"))
Check "saved file keeps CRLF" ($saved.Contains("first line`r`n") -and -not ($saved -match "[^`r]`n"))
Check "title clean after save" (-not (Title).StartsWith("*"))

# clipboard: select all + copy
Keys "^a"
Keys "^c"
Start-Sleep -Milliseconds 300
$clip = Get-Clipboard -Raw
Check "copy puts the document on the clipboard" ($clip -ne $null -and $clip.Contains("Hello 世界 ä"))
Keys "{RIGHT}"

# paste from the system clipboard
Set-Clipboard -Value "pasted 剪贴板"
Keys "^v"
Shot "03-pasted"

# find
Keys "^f"
Keys "Smoke"
Shot "04-find"
Keys "{ESC}"

# replace (Ctrl+H)
Keys "^h"
Shot "05-replace"
Keys "{ESC}"

# dark mode
Keys "^+l"
Shot "06-dark"
Keys "^+l"

# shortcuts sheet
Keys "^/"
Shot "07-shortcuts"
Keys "{ESC}"

# ⋯ menu › 打开… with the mouse: the dialog's modal loop paints the window
# while the click is still being dispatched (crashed before the runtime
# deferred rebuilds during dispatch). Cancel, then the app must still run.
Click 169 26 -FromRight
Start-Sleep -Milliseconds 500
Shot "07a-menu"
Click 300 127 -FromRight
Start-Sleep -Milliseconds 1200
ScreenShot "07a-open-dialog-from-menu"
Keys "{ESC}"
Start-Sleep -Milliseconds 800
Check "menu › 打开… then cancel keeps the app running" (!$p.HasExited -and [W]::IsWindow($h))

# open another file through the native open dialog (IFileOpenDialog)
$other = Join-Path $out "other.md"
[IO.File]::WriteAllText($other, "# Other file`n", (New-Object Text.UTF8Encoding $false))
Keys "^s"
Keys "^o"
Start-Sleep -Milliseconds 1200
ScreenShot "07b-open-dialog"
Keys $other
Keys "{ENTER}"
Start-Sleep -Milliseconds 800
Shot "07c-opened-other"
Check "open dialog opens the chosen file" ((Title) -like "still*" -and (Read-Text $other).Contains("Other"))

# save as (IFileSaveDialog, default extension .md)
$copy = Join-Path $out "copy"
Keys "^+s"
Start-Sleep -Milliseconds 1200
ScreenShot "07d-save-dialog"
Keys "^a"
Keys $copy
Keys "{ENTER}"
Start-Sleep -Milliseconds 900
Check "save as writes copy.md" (Test-Path "$copy.md")

# Microsoft Pinyin: compose "nihao", commit with space
Click 500 400
Keys "^{END}"
[W]::PostMessage($h, 0x50, [IntPtr]::Zero, [W]::LoadKeyboardLayout("00000804", 1)) | Out-Null
Start-Sleep -Milliseconds 700
[Windows.Forms.SendKeys]::SendWait("nihao"); Start-Sleep -Milliseconds 700
ScreenShot "07e-ime-composing"
Keys " "
Start-Sleep -Milliseconds 500
Shot "07f-ime-committed"
[W]::PostMessage($h, 0x50, [IntPtr]::Zero, [W]::LoadKeyboardLayout("00000409", 1)) | Out-Null
Start-Sleep -Milliseconds 400
Keys "^s"
Start-Sleep -Milliseconds 600
Check "pinyin IME commits 你好" ((Read-Text "$copy.md").Contains("你好"))

# preferences persisted in %APPDATA%
Check "settings.db in %APPDATA%" (Test-Path (Join-Path $env:APPDATA "com.racall.stillmd\settings.db"))

# caption buttons: maximize, then restore
$wasZoomed = [W]::IsZoomed($h)
Click (46 * 1.5) 20 -FromRight
Start-Sleep -Milliseconds 600
Check "maximize button maximizes" ([W]::IsZoomed($h) -and -not $wasZoomed)
Shot "08-maximized"
Click (46 * 1.5) 20 -FromRight
Start-Sleep -Milliseconds 600
Check "restore button restores" (-not [W]::IsZoomed($h))

# drag the empty title bar: the window moves
$before = Rect
$pt = ClientPoint 520 26
[W]::SetCursorPos($pt[0], $pt[1]) | Out-Null; Start-Sleep -Milliseconds 200
[W]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 200
for ($i = 1; $i -le 10; $i++) { [W]::SetCursorPos($pt[0] + $i * 12, $pt[1] + $i * 6) | Out-Null; Start-Sleep -Milliseconds 30 }
[W]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 400
$after = Rect
Check "dragging the title bar moves the window" (($after.L - $before.L) -ge 100 -and ($after.T - $before.T) -ge 50)

# double click the title bar: maximize, again: restore
$pt = ClientPoint 520 26
[W]::SetCursorPos($pt[0], $pt[1]) | Out-Null; Start-Sleep -Milliseconds 200
for ($k = 0; $k -lt 2; $k++) { [W]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero); [W]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 60 }
Start-Sleep -Milliseconds 600
Check "double click on the title bar maximizes" ([W]::IsZoomed($h))
$pt = ClientPoint 520 26
[W]::SetCursorPos($pt[0], $pt[1]) | Out-Null; Start-Sleep -Milliseconds 200
for ($k = 0; $k -lt 2; $k++) { [W]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero); [W]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 60 }
Start-Sleep -Milliseconds 600
Check "double click again restores" (-not [W]::IsZoomed($h))

# hover the close button (red)
$pt = ClientPoint 23 20 -FromRight
[W]::SetCursorPos($pt[0], $pt[1]) | Out-Null
Shot "09-close-hover"

# unsaved change + close: the app asks (TaskDialog); cancel keeps the window
Click 500 400
Keys "x"
Keys "%{F4}"
Start-Sleep -Milliseconds 800
ScreenShot "10-close-confirm"
Keys "{ESC}"
Start-Sleep -Milliseconds 500
Check "cancel keeps the window open" ([W]::IsWindow($h))

# close via the caption close button, answer "不保存" (second button)
Click 23 20 -FromRight
Start-Sleep -Milliseconds 800
ScreenShot "11-close-confirm-2"
Keys "{TAB}"
Keys "{ENTER}"
Start-Sleep -Milliseconds 800
Check "don't save closes the app" ($p.HasExited)
$final = Read-Text $doc
Check "don't save leaves the file untouched" (-not $final.EndsWith("x"))
if (!$p.HasExited) { Stop-Process -Id $p.Id -Force }

# Ctrl+W closes a window without unsaved changes right away
$p2 = Start-Process -FilePath $App -ArgumentList "`"$other`"" -PassThru
$h = [IntPtr]::Zero
for ($i = 0; $i -lt 50 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 200; $p2.Refresh(); $h = $p2.MainWindowHandle }
Start-Sleep -Milliseconds 800
[W]::SetForegroundWindow($h) | Out-Null
Keys "^w"
Start-Sleep -Milliseconds 800
Check "Ctrl+W closes a saved document window" ($p2.HasExited)
if (!$p2.HasExited) { Stop-Process -Id $p2.Id -Force }
Log "done"
