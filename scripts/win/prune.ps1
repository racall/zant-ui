# Deletes source files of a synced checkout that the Mac side no longer has
# (moved or deleted files): sync.sh only adds and overwrites, and a stale
# `foo.rs` next to a new `foo/mod.rs` breaks the build. Run by sync.sh after
# unpacking, from the checkout's root, with the manifest it shipped.
#
#   powershell -File scripts/win/prune.ps1 -Manifest .zant-sync-manifest
#
# Only source-like files under crates/, scaffold/, scripts/ and docs/ are
# considered; build output (any `target` directory) is never touched.
param([Parameter(Mandatory = $true)][string]$Manifest)
$ErrorActionPreference = "Stop"
$root = (Get-Location).Path
$keep = @{}
foreach ($line in Get-Content -Encoding UTF8 $Manifest) {
    if ($line) { $keep[$line] = $true }
}
$extensions = @(".rs", ".toml", ".ts", ".tsx", ".m", ".mm", ".c", ".cpp", ".h", ".md", ".json", ".sh", ".ps1", ".py")
$removed = 0
foreach ($dir in @("crates", "scaffold", "scripts", "docs")) {
    $base = Join-Path $root $dir
    if (!(Test-Path $base)) { continue }
    Get-ChildItem $base -Recurse -File | Where-Object {
        $_.FullName -notmatch "[\\/]target[\\/]" -and $extensions -contains $_.Extension
    } | ForEach-Object {
        $relative = $_.FullName.Substring($root.Length + 1).Replace("\", "/")
        if (!$keep.ContainsKey($relative)) {
            Remove-Item -Force $_.FullName
            Write-Output "pruned $relative"
            $removed++
        }
    }
}
if ($removed -gt 0) { Write-Output "prune.ps1: removed $removed stale file(s)" }
