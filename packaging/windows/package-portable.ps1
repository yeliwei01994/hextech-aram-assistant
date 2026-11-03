$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$outputDir = Join-Path $repoRoot 'outputs'
$packageVersion = (Get-Content (Join-Path $repoRoot 'package.json') -Raw | ConvertFrom-Json).version
$stageRoot = Join-Path $outputDir ('.portable-stage-' + [guid]::NewGuid().ToString('N'))
$portableDirName = "海克斯大乱斗小助手-$packageVersion-免安装版"
$portableDir = Join-Path $stageRoot $portableDirName
$archivePath = Join-Path $outputDir "HextechAramAssistant-$packageVersion-portable.zip"
$appExe = Join-Path $repoRoot 'src-tauri\target\release\aram-companion.exe'
$ocrRoot = Join-Path $repoRoot 'workers\ocr\dist\aram-ocr'
$ocrExe = Join-Path $ocrRoot 'aram-ocr.exe'
$tauriCli = Join-Path $repoRoot 'node_modules\.bin\tauri.cmd'

if (-not (Test-Path -LiteralPath $ocrExe)) {
    throw '找不到 OCR 组件，请先运行 pnpm ocr:build。'
}
if (-not (Test-Path -LiteralPath $tauriCli)) {
    throw '找不到 Tauri CLI，请先运行 pnpm install。'
}

New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
New-Item -ItemType Directory -Path $portableDir -Force | Out-Null
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

    Copy-Item -LiteralPath $appExe -Destination (Join-Path $portableDir 'aram-companion.exe')
    $ocrDestination = Join-Path $portableDir 'workers\ocr'
    New-Item -ItemType Directory -Path (Join-Path $ocrDestination 'dist\aram-ocr') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $repoRoot 'workers\ocr\card-layout.json') `
        -Destination (Join-Path $ocrDestination 'card-layout.json')
    Copy-Item -Path (Join-Path $ocrRoot '*') `
        -Destination (Join-Path $ocrDestination 'dist\aram-ocr') -Recurse -Force

    $readme = @'
海克斯大乱斗小助手 __APP_VERSION__ · 免安装版

启动
1. 将整个压缩包解压到一个固定文件夹，不要直接在压缩包内运行。
2. 双击 aram-companion.exe。
3. 程序在后台托盘运行；右键托盘图标可打开设置或退出。

运行环境
需要 Microsoft Edge WebView2 Runtime。若启动时提示缺少 WebView2，请安装微软运行时：
https://go.microsoft.com/fwlink/p/?LinkId=2124703

卸载
先右键托盘图标退出，再删除解压出来的整个文件夹即可。
应用设置和数据保存在当前 Windows 用户的本地数据目录，不会随程序文件夹删除。
'@
    $readme = $readme.Replace('__APP_VERSION__', $packageVersion)
    [IO.File]::WriteAllText(
        (Join-Path $portableDir '使用说明.txt'),
        $readme,
        [Text.UTF8Encoding]::new($true)
    )

    if (Test-Path -LiteralPath $archivePath) {
        Remove-Item -LiteralPath $archivePath -Force
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [System.IO.Compression.ZipFile]::CreateFromDirectory(
        $stageRoot,
        $archivePath,
        [System.IO.Compression.CompressionLevel]::Optimal,
        $false
    )

    $sizeMB = [Math]::Round((Get-Item -LiteralPath $archivePath).Length / 1MB, 1)
    Write-Host "免安装压缩包已生成：$archivePath ($sizeMB MB)"
}
finally {
    $resolvedStage = [IO.Path]::GetFullPath($stageRoot)
    $resolvedOutput = [IO.Path]::GetFullPath($outputDir) + [IO.Path]::DirectorySeparatorChar
    if ($resolvedStage.StartsWith($resolvedOutput, [StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $resolvedStage -Recurse -Force -ErrorAction SilentlyContinue
    }
}
