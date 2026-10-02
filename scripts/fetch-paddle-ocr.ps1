# Downloads ONNX Runtime and the PaddleOCR PP-OCRv5 mobile models, converted
# to ONNX by RapidOCR, into $Data\paddle-ocr: the runtime library, the text
# detector and a recogniser with its alphabet for each kind of script.
# MODELS picks the recognisers, by name:
#   latin cyrillic ch korean arabic devanagari el ta te th
# ch reads Chinese and Japanese. ORT_VERSION picks the runtime, 1.28 or newer.
. "$PSScriptRoot\common.ps1"

$Target = Join-Path $Data 'paddle-ocr'
$Models = if ($env:MODELS) { $env:MODELS } else { 'latin cyrillic ch korean arabic devanagari el ta te th' }
$Base = if ($env:BASE) { $env:BASE } else { 'https://www.modelscope.cn/models/RapidAI/RapidOCR/resolve/master' }
$OrtVersion = if ($env:ORT_VERSION) { $env:ORT_VERSION } else { '1.28.2' }

$work = New-WorkDirectory
try {
    $ort = "onnxruntime-win-x64-$OrtVersion"
    Get-File "https://github.com/microsoft/onnxruntime/releases/download/v$OrtVersion/$ort.zip" "$work\ort.zip"
    Expand-Archive -Path "$work\ort.zip" -DestinationPath "$work\ort"
    New-Item -ItemType Directory -Path "$work\models\rec" | Out-Null
    Copy-Item "$work\ort\$ort\lib\onnxruntime.dll" "$work\models\"

    Get-File "$Base/onnx/PP-OCRv5/det/ch_PP-OCRv5_det_mobile.onnx" "$work\models\det.onnx"
    foreach ($model in $Models -split '\s+' | Where-Object { $_ }) {
        $alphabet = if ($model -eq 'ch') { 'ppocrv5_dict.txt' } else { "ppocrv5_${model}_dict.txt" }
        Get-File "$Base/onnx/PP-OCRv5/rec/${model}_PP-OCRv5_rec_mobile.onnx" "$work\models\rec\$model.onnx"
        Get-File "$Base/paddle/PP-OCRv5/rec/${model}_PP-OCRv5_rec_mobile/$alphabet" "$work\models\rec\$model.txt"
    }
    New-Item -ItemType Directory -Force -Path $Target | Out-Null
    Copy-Item -Recurse -Force "$work\models\*" $Target
    Write-Host "PaddleOCR is in $Target"
} finally {
    Remove-Item -Recurse -Force $work
}
