$ErrorActionPreference = "Stop"
$projectRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$workerDir = Join-Path $projectRoot "workers\ocr"
$outputDir = Join-Path $workerDir "dist"
$workDir = Join-Path $workerDir "build"
$venvDir = Join-Path $workerDir ".venv"
$venvPython = Join-Path $venvDir "Scripts\python.exe"

if (-not (Test-Path -LiteralPath $venvPython)) {
  python -m venv $venvDir
  if ($LASTEXITCODE -ne 0) { throw "Failed to create the OCR virtual environment. Install Python and add it to PATH." }
}

& $venvPython --version
if ($LASTEXITCODE -ne 0) { throw "Python could not start inside the OCR virtual environment." }

& $venvPython -m pip install -r (Join-Path $workerDir "requirements.txt")
if ($LASTEXITCODE -ne 0) { throw "Failed to install OCR dependencies." }

& $venvPython -m PyInstaller `
  --noconfirm `
  --clean `
  --onedir `
  --name aram-ocr `
  --collect-all rapidocr `
  --collect-all onnxruntime `
  --collect-all mss `
  --collect-all PIL `
  --specpath $workDir `
  --distpath $outputDir `
  --workpath $workDir `
  (Join-Path $workerDir "worker.py")
if ($LASTEXITCODE -ne 0) { throw "PyInstaller failed to build the OCR worker." }

Write-Host "OCR worker created at $outputDir\aram-ocr\aram-ocr.exe"
