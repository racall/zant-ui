# Clicks every component of the zant-components example on the Windows
# desktop and checks the `[event]` output; screenshots go next to the report.
# Run in the logged-in session:
#   powershell -File scripts/win/run-desktop.ps1 -Exe powershell.exe `
#     -Arguments "-NoProfile -ExecutionPolicy Bypass -File <repo>\scripts\win\components-check.ps1" -Wait 600
param(
    [string]$Exe = "",
    [string]$Out = ""
)
# defaults here: $PSScriptRoot is empty inside param() on Windows PowerShell 5.1
if (!$Exe) { $Exe = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "zig-out\bin\zant-components.exe") }
if (!$Out) { $Out = (Join-Path $env:TEMP "zant-checks\components") }
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "ui-auto.ps1")
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Out
New-Item -ItemType Directory -Force $Out | Out-Null
$script:ResultFile = Join-Path $Out "report.txt"

# the example loads examples/assets/zant.png relative to the working directory
Set-Location (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
$app = Start-ZantApp $Exe $Out
Shot $app "00-initial"

# buttons, icon button, link
foreach ($c in @(
    @("btn-primary", "click primary"), @("btn-default", "click default"), @("btn-text", "click text"),
    @("btn-link", "click link-button"), @("btn-small", "click primary"), @("btn-large", "click default"),
    @("icon-btn", "click icon"), @("link", "click link"))) {
    Click-Key $app $c[0] | Out-Null
    Assert-Event $app $c[1] "button $($c[0])"
}
Click-Key $app "btn-disabled" | Out-Null
Start-Sleep -Milliseconds 200
Write-Result (@(New-Events $app).Count -eq 0) "disabled button ignores clicks"

# hover states and tooltips
Hover-Key $app "btn-primary"; Shot $app "01-hover-primary"
Hover-Key $app "tooltip-btn"; Start-Sleep -Milliseconds 900; Shot $app "02-tooltip"
Hover-Key $app "icon-btn"; Start-Sleep -Milliseconds 900; Shot $app "03-icon-tooltip"

# text input: typing (non-ASCII), submit; password field
Click-Key $app "input" | Out-Null
Send-Keys "hello 世界"
Assert-Event $app "input hello 世界" "input typing (incl. CJK)"
Send-Keys "{ENTER}"
Assert-Event $app "submit" "input submit on Enter"
Send-Keys "^a"; Send-Keys "{BACKSPACE}"
Assert-Event $app "input " "select all + delete clears the input"
Click-Key $app "input-password" | Out-Null
Send-Keys "secret"
Shot $app "04-inputs"

# checkbox, switch
Click-Key $app "checkbox" | Out-Null
Assert-Event $app "checkbox false" "checkbox unchecks"
Click-Key $app "checkbox" | Out-Null
Assert-Event $app "checkbox true" "checkbox checks"
Click-Key $app "switch" | Out-Null
Assert-Event $app "switch true" "switch turns on"

# radio group (option 2 is disabled)
Click-Key $app "radio-1" | Out-Null
Assert-Event $app "radio 1" "radio selects option 2"
Click-Key $app "radio-2" | Out-Null
Start-Sleep -Milliseconds 200
Write-Result (@(New-Events $app) -notcontains "radio 2") "disabled radio option ignores clicks"

# select: open, pick the third option (popup rows are 30px below the field)
$sel = Find-Key $app "select"
Click-Key $app "select" | Out-Null
Shot $app "05-select-open"
Click-At $app ($sel.X + $sel.W / 2) ($sel.Y + $sel.H + 8 + 2 * 30 + 15)
Assert-Event $app "select 2" "select picks an option"

# segmented control and tabs
Click-Key $app "segmented-2-hit" | Out-Null
Assert-Event $app "segmented 2" "segmented switches to item 3"
Click-Key $app "tabs-1" | Out-Null
Assert-Event $app "tab 1" "tabs switch"
Shot $app "06-nav"

# progress
Click-Key $app "progress-more" | Out-Null
Assert-Event $app "progress 60" "progress +20%"

# dropdown menu: open, pick an item; open again, dismiss by clicking outside
Click-Key $app "menu-btn" | Out-Null
Assert-Event $app "menu open" "dropdown opens"
Shot $app "07-menu"
$btn = Find-Key $app "menu-btn"; $menu = Find-Key $app "menu"
$aligned = $menu -and [math]::Abs($menu.X - $btn.X) -lt 1 -and ([math]::Abs($menu.Y - ($btn.Y + $btn.H + 4)) -lt 1 -or [math]::Abs($menu.Y + $menu.H - ($btn.Y - 4)) -lt 1)
Write-Result $aligned "dropdown sits right below (or above) its button"
Click-Key $app "menu-new" | Out-Null
Assert-Event $app "menu new" "dropdown item click"
Click-Key $app "menu-btn" | Out-Null
New-Events $app | Out-Null
Click-Key $app "menu-disabled" | Out-Null
Start-Sleep -Milliseconds 200
Write-Result (@(New-Events $app).Count -eq 0) "disabled menu item ignores clicks"
Click-At $app 40 40
Assert-Event $app "menu dismiss" "click outside closes the dropdown"

# modal: open, close with the footer button; open, close with Esc
Click-Key $app "modal-btn" | Out-Null
Assert-Event $app "modal open" "modal opens"
Shot $app "08-modal"
Click-Key $app "modal-ok" | Out-Null
Assert-Event $app "modal close" "modal footer button closes"
Click-Key $app "modal-btn" | Out-Null
New-Events $app | Out-Null
Send-Keys "{ESC}"
Assert-Event $app "escape" "Esc closes the modal"

# message toast
Click-Key $app "message-btn" | Out-Null
Assert-Event $app "message show" "message shows"
Shot $app "09-message"
Write-Result ((Find-Key $app "message") -ne $null) "message toast is on screen"
Start-Sleep -Milliseconds 3200
Write-Result ((Find-Key $app "message") -eq $null) "message toast goes away after 3 s"

# pointer area: drag the knob, double click resets it (scroll the page
# down first: the section is below the fold)
Wheel-At $app 500 400 -10
Start-Sleep -Milliseconds 300
$track = Find-Key $app "track"
if ($track) {
    Drag-From-To $app ($track.X + 20) ($track.Y + $track.H / 2) ($track.X + $track.W - 10) ($track.Y + $track.H / 2)
    Assert-Event $app "pointer up" "pointer area drag"
    Double-Click-At $app ($track.X + $track.W / 2) ($track.Y + $track.H / 2)
    Assert-Event $app "pointer double" "pointer area double click"
} else { Write-Result $false "find 'track'" }
# virtual list: the wheel scrolls 10 000 rows (only visible ones exist)
$vl = Find-Key $app "vlist"
if ($vl) {
    Wheel-At $app ($vl.X + $vl.W / 2) ($vl.Y + $vl.H / 2) -20
    Assert-Event $app "list rows" "virtual list scrolls with the wheel"
    Shot $app "09c-list"
} else { Write-Result $false "find 'vlist'" }
# selectable text: drag across both paragraphs, Ctrl+C
$sel = Find-Key $app "select-text"
if ($sel) {
    Set-Clipboard -Value "before"
    Drag-From-To $app ($sel.X + 2) ($sel.Y + 8) ($sel.X + $sel.W - 4) ($sel.Y + $sel.H - 4)
    Send-Keys "^c"
    $clip = Get-Clipboard -Raw
    Write-Result ($clip -ne $null -and $clip.Contains("selectable") -and $clip.Contains("双击")) "selectable text: drag + Ctrl+C copies both paragraphs"
    Shot $app "09b-selection"
} else { Write-Result $false "find 'select-text'" }
Wheel-At $app 500 400 10

# dark theme
Click-Key $app "dark-btn" | Out-Null
Assert-Event $app "dark true" "dark theme"
Shot $app "10-dark"
Click-Key $app "menu-btn" | Out-Null
Shot $app "11-dark-menu"
Send-Keys "{ESC}"

Write-Result (!$app.Process.HasExited) "app still running after all interactions"
Stop-ZantApp $app
Add-Content -Encoding UTF8 $script:ResultFile "done"
