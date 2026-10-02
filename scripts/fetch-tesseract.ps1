# Downloads Tesseract language models into $Data\tesseract. The library comes
# from the Tesseract installer of UB Mannheim
# (https://github.com/UB-Mannheim/tesseract/wiki), into its default folder,
# C:\Program Files\Tesseract-OCR; without models here, the program reads those
# the installer put there.
# LANGUAGES picks the models, by Tesseract's names (eng rus deu chi_sim ...);
# English is read with every other language. MODELS picks fast or best:
# best reads a little better, several times slower.
. "$PSScriptRoot\common.ps1"

$Target = Join-Path $Data 'tesseract'
$Languages = if ($env:LANGUAGES) { $env:LANGUAGES } else { 'eng rus' }
$Models = if ($env:MODELS) { $env:MODELS } else { 'fast' }
if ($Models -notin 'fast', 'best') { throw "MODELS is fast or best, not $Models" }

if (-not (Test-Path 'C:\Program Files\Tesseract-OCR\libtesseract-5.dll')) {
    Write-Warning 'Tesseract is not installed in C:\Program Files\Tesseract-OCR'
}

$work = New-WorkDirectory
try {
    foreach ($language in $Languages -split '\s+' | Where-Object { $_ }) {
        Get-File "https://raw.githubusercontent.com/tesseract-ocr/tessdata_$Models/main/$language.traineddata" `
            "$work\$language.traineddata"
    }
    New-Item -ItemType Directory -Force -Path $Target | Out-Null
    Copy-Item -Force "$work\*.traineddata" $Target
    Write-Host "Tesseract models are in $Target"
} finally {
    Remove-Item -Recurse -Force $work
}
