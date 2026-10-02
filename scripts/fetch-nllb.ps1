# Converts Meta NLLB-200 for CTranslate2 into $Data\nllb, with its tokenizer.
# The converter runs in a throwaway Python environment; MODEL picks the model.
# Needs Python 3 (the py launcher or python on PATH).
# The NLLB-200 weights are licensed CC-BY-NC 4.0, for non-commercial use only.
. "$PSScriptRoot\common.ps1"

$Target = Join-Path $Data 'nllb'
$Model = if ($env:MODEL) { $env:MODEL } else { 'facebook/nllb-200-distilled-600M' }
$Quantization = if ($env:QUANTIZATION) { $env:QUANTIZATION } else { 'int8' }

$python = if (Get-Command py -ErrorAction SilentlyContinue) { 'py' } else { 'python' }

$work = New-WorkDirectory
try {
    & $python -m venv "$work\venv"
    if ($LASTEXITCODE) { throw 'cannot make a Python environment' }
    & "$work\venv\Scripts\python.exe" -m pip install -q --upgrade pip
    & "$work\venv\Scripts\pip.exe" install -q ctranslate2 transformers sentencepiece `
        torch --extra-index-url https://download.pytorch.org/whl/cpu
    if ($LASTEXITCODE) { throw 'cannot install the converter' }

    Write-Host "converting $Model"
    & "$work\venv\Scripts\ct2-transformers-converter.exe" --model $Model `
        --quantization $Quantization --copy_files tokenizer.json `
        --output_dir "$work\nllb"
    if ($LASTEXITCODE) { throw 'conversion failed' }
    New-Item -ItemType Directory -Force -Path $Target | Out-Null
    Copy-Item -Recurse -Force "$work\nllb\*" $Target
    Write-Host "NLLB-200 is in $Target"
} finally {
    Remove-Item -Recurse -Force $work
}
