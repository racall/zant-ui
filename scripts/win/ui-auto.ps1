# UI automation helpers for Zant UI apps on the Windows desktop (dot-source
# it from a test script that runs in the logged-in session). Apps run with
# ZANT_LAYOUT_FILE, so widgets are found by their `key`, not by guessed
# coordinates; stderr is captured so `[event] ...` lines can be checked.
#
#   . $PSScriptRoot\ui-auto.ps1
#   $app = Start-ZantApp "<repo>\zig-out\bin\zant-components.exe" -Out $out
#   Click-Key $app "btn-primary"
#   Assert-Event $app "click primary" "primary button"
#   Stop-ZantApp $app
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
if (-not ("ZantAuto" -as [type])) {
    Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ZantAuto {
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@
}
[ZantAuto]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null

function Start-ZantApp([string]$Exe, [string]$Out, [string]$Arguments = "", [hashtable]$Env = @{}, [switch]$Motion) {
    $layout = Join-Path $Out "layout.txt"
    $stderr = Join-Path $Out "app-stderr.txt"
    Remove-Item -Force -ErrorAction SilentlyContinue $layout, $stderr
    $env:ZANT_LAYOUT_FILE = $layout
    # animations off by default so screenshots are deterministic
    $env:ZANT_NO_MOTION = if ($Motion) { "" } else { "1" }
    foreach ($k in $Env.Keys) { Set-Item "env:$k" $Env[$k] }
    $startArgs = @{ FilePath = $Exe; PassThru = $true; RedirectStandardError = $stderr; RedirectStandardOutput = (Join-Path $Out "app-stdout.txt") }
    if ($Arguments) { $startArgs.ArgumentList = $Arguments }
    $p = Start-Process @startArgs
    $h = [IntPtr]::Zero
    for ($i = 0; $i -lt 60 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 200; $p.Refresh(); $h = $p.MainWindowHandle }
    for ($i = 0; $i -lt 50 -and !(Test-Path $layout); $i++) { Start-Sleep -Milliseconds 100 }
    Start-Sleep -Milliseconds 500
    [ZantAuto]::SetForegroundWindow($h) | Out-Null
    [pscustomobject]@{ Process = $p; Hwnd = $h; Layout = $layout; Stderr = $stderr; Out = $Out; Scale = [ZantAuto]::GetDpiForWindow($h) / 96.0; Seen = 0 }
}

function Stop-ZantApp($app) {
    if (!$app.Process.HasExited) { Stop-Process -Id $app.Process.Id -Force }
}

# bounds (logical px) of the node with `key`, from the latest frame
function Find-Key($app, [string]$key) {
    for ($i = 0; $i -lt 20; $i++) {
        $lines = Get-Content -ErrorAction SilentlyContinue $app.Layout
        foreach ($line in $lines) {
            $f = $line -split "`t"
            if ($f[0] -eq $key) { return [pscustomobject]@{ X = [double]$f[1]; Y = [double]$f[2]; W = [double]$f[3]; H = [double]$f[4] } }
        }
        Start-Sleep -Milliseconds 100
    }
    return $null
}

function Move-To($app, [double]$lx, [double]$ly) {
    $o = New-Object ZantAuto+POINT
    [ZantAuto]::ClientToScreen($app.Hwnd, [ref]$o) | Out-Null
    [ZantAuto]::SetCursorPos($o.X + [int]($lx * $app.Scale), $o.Y + [int]($ly * $app.Scale)) | Out-Null
}

function Click-At($app, [double]$lx, [double]$ly) {
    Move-To $app $lx $ly
    Start-Sleep -Milliseconds 150
    [ZantAuto]::mouse_event(0x2, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 60
    [ZantAuto]::mouse_event(0x4, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 350
}

# click the centre of a keyed node (or a point inside it, fx/fy in 0..1)
function Click-Key($app, [string]$key, [double]$fx = 0.5, [double]$fy = 0.5) {
    $b = Find-Key $app $key
    if (!$b) { Write-Result $false "find '$key'"; return $false }
    Click-At $app ($b.X + $b.W * $fx) ($b.Y + $b.H * $fy)
    return $true
}

function Hover-Key($app, [string]$key) {
    $b = Find-Key $app $key
    if ($b) { Move-To $app ($b.X + $b.W / 2) ($b.Y + $b.H / 2) }
}

function Send-Keys([string]$keys) {
    [Windows.Forms.SendKeys]::SendWait($keys)
    Start-Sleep -Milliseconds 250
}

function Shot($app, [string]$name) {
    Start-Sleep -Milliseconds 300
    $c = New-Object ZantAuto+RECT; [ZantAuto]::GetClientRect($app.Hwnd, [ref]$c) | Out-Null
    $o = New-Object ZantAuto+POINT; [ZantAuto]::ClientToScreen($app.Hwnd, [ref]$o) | Out-Null
    $bmp = New-Object Drawing.Bitmap $c.R, $c.B
    $g = [Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($o.X, $o.Y, 0, 0, $bmp.Size)
    $bmp.Save((Join-Path $app.Out "$name.png"))
    $g.Dispose(); $bmp.Dispose()
}

# `[event] ...` lines the app printed since the last call
function New-Events($app) {
    $lines = @(Get-Content -Encoding UTF8 -ErrorAction SilentlyContinue $app.Stderr | Where-Object { $_ -like "``[event``]*" })
    $new = if ($lines.Count -gt $app.Seen) { $lines[$app.Seen..($lines.Count - 1)] } else { @() }
    $app.Seen = $lines.Count
    $new | ForEach-Object { $_.Substring(8) }
}

$script:ResultFile = $null
function Write-Result([bool]$ok, [string]$name) {
    $line = "{0} {1}" -f $(if ($ok) { "PASS" } else { "FAIL" }), $name
    if ($script:ResultFile) { Add-Content -Encoding UTF8 $script:ResultFile $line } else { Write-Host $line }
}

# passes when the app printed `[event] <expected>` since the last check
function Assert-Event($app, [string]$expected, [string]$name) {
    Start-Sleep -Milliseconds 150
    $events = @(New-Events $app)
    $ok = $events -contains $expected
    $detail = if ($ok) { "" } else { "  (got: " + ($events -join " | ") + ")" }
    Write-Result $ok "$name$detail"
}
