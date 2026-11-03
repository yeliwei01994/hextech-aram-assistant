$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$outputDir = Join-Path $repoRoot 'outputs'
$packageVersion = (Get-Content (Join-Path $repoRoot 'package.json') -Raw | ConvertFrom-Json).version
$stageRoot = Join-Path $outputDir ('.windows-package-' + [guid]::NewGuid().ToString('N'))
$payloadDir = Join-Path $stageRoot 'payload'
$payloadZip = Join-Path $stageRoot 'payload.zip'
$installerPath = Join-Path $outputDir "HextechAramAssistant-$packageVersion-setup.exe"
$sedPath = Join-Path $stageRoot 'installer.sed'
$appExe = Join-Path $repoRoot 'src-tauri\target\release\aram-companion.exe'
$ocrRoot = Join-Path $repoRoot 'workers\ocr\dist\aram-ocr'
$ocrExe = Join-Path $ocrRoot 'aram-ocr.exe'
$tauriCli = Join-Path $repoRoot 'node_modules\.bin\tauri.cmd'
$iexpress = Join-Path $env:WINDIR 'System32\iexpress.exe'

if (-not (Test-Path -LiteralPath $iexpress)) {
    throw '当前 Windows 缺少 IExpress，无法生成自解压安装包。'
}
if (-not (Test-Path -LiteralPath $ocrExe)) {
    throw 'OCR 组件尚未构建，请先运行 pnpm ocr:build。'
}
if (-not (Test-Path -LiteralPath $tauriCli)) {
    throw '找不到 Tauri CLI，请先运行 pnpm install。'
}

New-Item -ItemType Directory -Path $stageRoot -Force | Out-Null
try {
    Push-Location $repoRoot
    try {
        & $tauriCli build --no-bundle
        if ($LASTEXITCODE -ne 0) { throw "Tauri 构建失败，退出代码：$LASTEXITCODE" }
    }
    finally {
        Pop-Location
    }

    if (-not (Test-Path -LiteralPath $appExe)) {
        throw "构建结束后找不到程序：$appExe"
    }

    New-Item -ItemType Directory -Path $payloadDir -Force | Out-Null
    Copy-Item -LiteralPath $appExe -Destination (Join-Path $payloadDir 'aram-companion.exe')
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'uninstall.ps1') -Destination (Join-Path $payloadDir 'uninstall.ps1')

    $resourceRoot = Join-Path $payloadDir 'workers\ocr'
    New-Item -ItemType Directory -Path $resourceRoot -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $repoRoot 'workers\ocr\card-layout.json') `
        -Destination (Join-Path $resourceRoot 'card-layout.json')
    $ocrDestination = Join-Path $resourceRoot 'dist\aram-ocr'
    New-Item -ItemType Directory -Path $ocrDestination -Force | Out-Null
    Copy-Item -Path (Join-Path $ocrRoot '*') -Destination $ocrDestination -Recurse -Force

    Compress-Archive -Path (Join-Path $payloadDir '*') `
        -DestinationPath $payloadZip -CompressionLevel Optimal
    $installerScript = (Get-Content (Join-Path $PSScriptRoot 'install.ps1') -Raw).Replace('__APP_VERSION__', $packageVersion)
    [IO.File]::WriteAllText(
        (Join-Path $stageRoot 'install.ps1'),
        $installerScript,
        [Text.UTF8Encoding]::new($true)
    )

    $sedContents = @"
[Version]
Class=IEXPRESS
SEDVersion=3
[Options]
PackagePurpose=InstallApp
ShowInstallProgramWindow=0
HideExtractAnimation=0
UseLongFileName=1
InsideCompressed=1
CAB_FixedSize=0
CAB_ResvCodeSigning=0
RebootMode=N
InstallPrompt=
DisplayLicense=
FinishMessage=
TargetName=$installerPath
FriendlyName=Hextech ARAM Assistant
AppLaunched=powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File install.ps1
PostInstallCmd=<None>
AdminQuietInstCmdLine=
UserQuietInstCmdLine=
SourceFiles=SourceFiles
[Strings]
FILE0="install.ps1"
FILE1="payload.zip"
[SourceFiles]
SourceFiles0=$stageRoot
[SourceFiles0]
%FILE0%=
%FILE1%=
"@
    [IO.File]::WriteAllText($sedPath, $sedContents, [Text.Encoding]::ASCII)

    Write-Host '正在压缩 Windows 安装包，这一步可能需要一两分钟……'
    if (Test-Path -LiteralPath $installerPath) {
        Remove-Item -LiteralPath $installerPath -Force
    }
    $iexpressArguments = @('/N', '/Q', $sedPath)
    $iexpressProcess = Start-Process -FilePath $iexpress `
        -ArgumentList $iexpressArguments -Wait -PassThru
    if ($iexpressProcess.ExitCode -ne 0) {
        throw "IExpress 打包失败，退出代码：$($iexpressProcess.ExitCode)"
    }
    if (-not (Test-Path -LiteralPath $installerPath)) {
        throw 'IExpress 没有生成安装包。'
    }

    $sizeMB = [Math]::Round((Get-Item -LiteralPath $installerPath).Length / 1MB, 1)
    Write-Host "Windows 安装包已生成：$installerPath ($sizeMB MB)"
}
finally {
    $resolvedStage = [IO.Path]::GetFullPath($stageRoot)
    $resolvedOutput = [IO.Path]::GetFullPath($outputDir) + [IO.Path]::DirectorySeparatorChar
    if ($resolvedStage.StartsWith($resolvedOutput, [StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $resolvedStage -Recurse -Force -ErrorAction SilentlyContinue
    }
}
