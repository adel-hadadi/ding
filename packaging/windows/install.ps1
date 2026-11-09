# Put Ding in the Start menu so you can open it without a terminal.
$ErrorActionPreference = "Stop"

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$exe = Join-Path $here "ding-gui.exe"
if (-not (Test-Path $exe)) {
    Write-Error "ding-gui.exe is missing. Unzip the Windows release next to this script."
}

$dest = Join-Path $env:LOCALAPPDATA "Ding"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item $exe (Join-Path $dest "ding-gui.exe") -Force

$cli = Join-Path $here "ding.exe"
if (Test-Path $cli) {
    Copy-Item $cli (Join-Path $dest "ding.exe") -Force
}

$programs = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$shortcutPath = Join-Path $programs "Ding.lnk"
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = Join-Path $dest "ding-gui.exe"
$shortcut.WorkingDirectory = $dest
$shortcut.WindowStyle = 1
$shortcut.Description = "Ding a coworker on the LAN"
$shortcut.Save()

Write-Host "Ding is in the Start menu. Search for Ding and open it."
Write-Host "Installed: $dest"
