$ErrorActionPreference = "Stop"
$projectRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$workerDir = Join-Path $projectRoot "workers\ocr"
$outputDir = Join-Path $workerDir "dist"
$workDir = Join-Path $workerDir "build"
$venvDir = Join-Path $workerDir ".venv"
$venvPython = Join-Path $venvDir "Scripts\python.exe"

function Test-OcrPython($path) {
  if (-not (Test-Path -LiteralPath $path)) { return $false }
  try {
    & $path --version 2>$null | Out-Null
    return $LASTEXITCODE -eq 0
  } catch {
    return $false
  }
}

if (-not (Test-OcrPython $venvPython)) {
  if (Test-Path -LiteralPath $venvDir) {
    $resolvedWorkerDir = (Resolve-Path -LiteralPath $workerDir).Path.TrimEnd('\') + '\'
    $resolvedVenvDir = [System.IO.Path]::GetFullPath($venvDir).TrimEnd('\') + '\'
    if (-not $resolvedVenvDir.StartsWith($resolvedWorkerDir, [System.StringComparison]::OrdinalIgnoreCase)) {
      throw "Refusing to remove an OCR environment outside $workerDir."
    }
    Remove-Item -LiteralPath $venvDir -Recurse -Force
  }

  $candidates = @()
  if ($env:OCR_PYTHON -and (Test-Path -LiteralPath $env:OCR_PYTHON)) {
    $candidates += [pscustomobject]@{ Path = $env:OCR_PYTHON; Prefix = @() }
  }
  foreach ($command in @(Get-Command py -All -ErrorAction SilentlyContinue)) {
    $candidates += [pscustomobject]@{ Path = $command.Source; Prefix = @('-3') }
  }
  foreach ($name in @('python', 'python3')) {
    foreach ($command in @(Get-Command $name -All -ErrorAction SilentlyContinue)) {
      if ($command.Source -notlike '*\WindowsApps\*') {
        $candidates += [pscustomobject]@{ Path = $command.Source; Prefix = @() }
      }
    }
  }

  $created = $false
  foreach ($candidate in $candidates) {
    & $candidate.Path @($candidate.Prefix) -m venv $venvDir
    if ($LASTEXITCODE -eq 0 -and (Test-OcrPython $venvPython)) {
      $created = $true
      break
    }
    if (Test-Path -LiteralPath $venvDir) { Remove-Item -LiteralPath $venvDir -Recurse -Force }
  }
  if (-not $created) {
    throw "Failed to create the OCR virtual environment. Install Python, set OCR_PYTHON, or add Python to PATH."
  }
}

& $venvPython -m pip install -r (Join-Path $workerDir "requirements.txt")
if ($LASTEXITCODE -ne 0) { throw "Failed to install OCR dependencies." }

$pythonBasePrefix = (& $venvPython -c "import sys; print(sys.base_prefix)").Trim()
$pythonLibraryBin = Join-Path $pythonBasePrefix "Library\bin"
if (Test-Path -LiteralPath $pythonLibraryBin) {
  $env:PATH = "$pythonLibraryBin;$env:PATH"
}

$pyinstallerArgs = @(
  "--noconfirm", "--clean", "--onedir", "--name", "aram-ocr",
  "--collect-all", "rapidocr", "--collect-all", "onnxruntime",
  "--collect-all", "mss", "--collect-all", "PIL",
  "--specpath", $workDir, "--distpath", $outputDir, "--workpath", $workDir
)
$ffiDll = Join-Path $pythonLibraryBin "ffi.dll"
if (Test-Path -LiteralPath $ffiDll) {
  $pyinstallerArgs += @("--add-binary", "$ffiDll;.")
}
$pyinstallerArgs += (Join-Path $workerDir "worker.py")

& $venvPython -m PyInstaller @pyinstallerArgs
if ($LASTEXITCODE -ne 0) { throw "PyInstaller failed to build the OCR worker." }

Write-Host "OCR worker created at $outputDir\aram-ocr\aram-ocr.exe"
