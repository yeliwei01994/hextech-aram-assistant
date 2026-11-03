$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms

$installDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$startMenuDir = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\海克斯大乱斗小助手'
$desktopShortcut = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::DesktopDirectory)) '海克斯大乱斗小助手.lnk'

$answer = [System.Windows.Forms.MessageBox]::Show(
    '确定卸载海克斯大乱斗小助手吗？本地偏好和数据会保留。',
    '卸载海克斯大乱斗小助手',
    [System.Windows.Forms.MessageBoxButtons]::YesNo,
    [System.Windows.Forms.MessageBoxIcon]::Question
)
if ($answer -ne [System.Windows.Forms.DialogResult]::Yes) { exit 0 }

Get-Process -Name 'aram-companion' -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $startMenuDir -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $desktopShortcut -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\HextechAramAssistant' `
    -Recurse -Force -ErrorAction SilentlyContinue

$escapedInstallDir = $installDir.Replace("'", "''")
$removeCommand = "Start-Sleep -Seconds 2; Remove-Item -LiteralPath '$escapedInstallDir' -Recurse -Force -ErrorAction SilentlyContinue"
Start-Process -FilePath 'powershell.exe' `
    -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', $removeCommand) `
    -WindowStyle Hidden
[System.Windows.Forms.MessageBox]::Show(
    '已提交卸载。本地数据和偏好设置已保留。',
    '海克斯大乱斗小助手',
    [System.Windows.Forms.MessageBoxButtons]::OK,
    [System.Windows.Forms.MessageBoxIcon]::Information
) | Out-Null
