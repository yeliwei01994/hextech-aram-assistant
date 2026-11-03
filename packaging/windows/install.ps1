$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Windows.Forms

function Show-InstallError([string] $message) {
    [System.Windows.Forms.MessageBox]::Show(
        $message,
        '海克斯大乱斗小助手安装失败',
        [System.Windows.Forms.MessageBoxButtons]::OK,
        [System.Windows.Forms.MessageBoxIcon]::Error
    ) | Out-Null
}

function Test-WebView2Runtime {
    $programFilesX86 = [Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
    $roots = @(
        (Join-Path $env:LOCALAPPDATA 'Microsoft\EdgeWebView\Application'),
        (Join-Path $env:ProgramFiles 'Microsoft\EdgeWebView\Application'),
        (Join-Path $programFilesX86 'Microsoft\EdgeWebView\Application')
    ) | Where-Object { $_ }

    foreach ($root in $roots) {
        if (Test-Path -LiteralPath $root) {
            $runtime = Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue |
                Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'msedgewebview2.exe') } |
                Select-Object -First 1
            if ($runtime) { return $true }
        }
    }
    return $false
}

try {
    $payloadZip = Join-Path $PSScriptRoot 'payload.zip'
    if (-not (Test-Path -LiteralPath $payloadZip)) {
        throw '安装包文件不完整，请重新下载后再试。'
    }

    if (-not (Test-WebView2Runtime)) {
        $bootstrapper = Join-Path $env:TEMP ('webview2-' + [guid]::NewGuid().ToString('N') + '.exe')
        try {
            Invoke-WebRequest -UseBasicParsing `
                -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' `
                -OutFile $bootstrapper
            $webviewInstall = Start-Process -FilePath $bootstrapper `
                -ArgumentList '/silent', '/install' -Wait -PassThru
            if ($webviewInstall.ExitCode -ne 0) {
                throw "WebView2 安装程序返回代码 $($webviewInstall.ExitCode)。"
            }
            if (-not (Test-WebView2Runtime)) {
                throw '没有检测到 WebView2 运行时。'
            }
        }
        catch {
            throw "需要安装 Microsoft WebView2 运行时，但自动安装失败。请检查网络后重试。`n$($_.Exception.Message)"
        }
        finally {
            Remove-Item -LiteralPath $bootstrapper -Force -ErrorAction SilentlyContinue
        }
    }

    $installDir = Join-Path $env:LOCALAPPDATA 'Programs\HextechAramAssistant'
    $startMenuDir = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\海克斯大乱斗小助手'
    $desktopPath = [Environment]::GetFolderPath([Environment+SpecialFolder]::DesktopDirectory)
    $exePath = Join-Path $installDir 'aram-companion.exe'
    $uninstallPath = Join-Path $installDir 'uninstall.ps1'
    $uninstallKeyPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\HextechAramAssistant'

    Get-Process -Name 'aram-companion' -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue

    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Expand-Archive -LiteralPath $payloadZip -DestinationPath $installDir -Force

    if (-not (Test-Path -LiteralPath $exePath)) {
        throw '安装包中缺少主程序，请重新下载后再试。'
    }

    New-Item -ItemType Directory -Path $startMenuDir -Force | Out-Null
    $shell = New-Object -ComObject WScript.Shell
    foreach ($shortcutPath in @(
        (Join-Path $startMenuDir '海克斯大乱斗小助手.lnk'),
        (Join-Path $desktopPath '海克斯大乱斗小助手.lnk')
    )) {
        $shortcut = $shell.CreateShortcut($shortcutPath)
        $shortcut.TargetPath = $exePath
        $shortcut.WorkingDirectory = $installDir
        $shortcut.IconLocation = "$exePath,0"
        $shortcut.Save()
    }

    New-Item -Path $uninstallKeyPath -Force | Out-Null
    New-ItemProperty -Path $uninstallKeyPath -Name DisplayName `
        -Value '海克斯大乱斗小助手' -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $uninstallKeyPath -Name DisplayVersion `
        -Value '__APP_VERSION__' -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $uninstallKeyPath -Name Publisher `
        -Value 'Hextech ARAM Assistant' -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $uninstallKeyPath -Name InstallLocation `
        -Value $installDir -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $uninstallKeyPath -Name UninstallString `
        -Value "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$uninstallPath`"" `
        -PropertyType String -Force | Out-Null

    Start-Process -FilePath $exePath -WorkingDirectory $installDir
    [System.Windows.Forms.MessageBox]::Show(
        '安装完成，助手已启动并常驻系统托盘。右键托盘图标可打开设置。',
        '海克斯大乱斗小助手',
        [System.Windows.Forms.MessageBoxButtons]::OK,
        [System.Windows.Forms.MessageBoxIcon]::Information
    ) | Out-Null
}
catch {
    Show-InstallError $_.Exception.Message
    exit 1
}
