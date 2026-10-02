#!/bin/sh
# Downloads ONNX Runtime and the PaddleOCR PP-OCRv5 mobile models, converted
# to ONNX by RapidOCR, into $DATA/paddle-ocr: the runtime library, the text
# detector and a recogniser with its alphabet for each kind of script.
# MODELS picks the recognisers, by name:
#   latin cyrillic ch korean arabic devanagari el ta te th
# ch reads Chinese and Japanese. ORT_VERSION picks the runtime, 1.28 or newer.
set -eu
. "$(dirname "$0")/common.sh"

TARGET=$DATA/paddle-ocr
MODELS=${MODELS:-latin cyrillic ch korean arabic devanagari el ta te th}
BASE=${BASE:-https://www.modelscope.cn/models/RapidAI/RapidOCR/resolve/master}
ORT_VERSION=${ORT_VERSION:-1.28.2}

case $(uname -m) in
    x86_64) ORT_ARCH=x64 ;;
    aarch64 | arm64) ORT_ARCH=aarch64 ;;
    *) die "no ONNX Runtime build for $(uname -m); put libonnxruntime.so into $TARGET" ;;
esac

need curl tar

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fetch() {
    echo "downloading $1"
    curl -fL --progress-bar "$BASE/$1" -o "$work/$2"
}

ort=onnxruntime-linux-$ORT_ARCH-$ORT_VERSION
echo "downloading $ort"
curl -fL --progress-bar \
    "https://github.com/microsoft/onnxruntime/releases/download/v$ORT_VERSION/$ort.tgz" |
    tar -xz -C "$work"
# The versioned file, not the link to it.
cp -L "$work/$ort/lib/libonnxruntime.so" "$work/libonnxruntime.so"
rm -rf "${work:?}/$ort"

mkdir -p "$work/rec"
fetch onnx/PP-OCRv5/det/ch_PP-OCRv5_det_mobile.onnx det.onnx
for model in $MODELS; do
    case $model in
        ch) alphabet=ppocrv5_dict.txt ;;
        *) alphabet=ppocrv5_${model}_dict.txt ;;
    esac
    fetch "onnx/PP-OCRv5/rec/${model}_PP-OCRv5_rec_mobile.onnx" "rec/$model.onnx"
    fetch "paddle/PP-OCRv5/rec/${model}_PP-OCRv5_rec_mobile/$alphabet" "rec/$model.txt"
done
mkdir -p "$TARGET"
cp -r "$work/." "$TARGET/"
echo "PaddleOCR is in $TARGET"
