#!/bin/sh
# Converts Meta NLLB-200 for CTranslate2 into $DATA/nllb, with its tokenizer.
# The converter runs in a throwaway Python environment; MODEL picks the model.
# The NLLB-200 weights are licensed CC-BY-NC 4.0, for non-commercial use only.
set -eu
. "$(dirname "$0")/common.sh"

TARGET=$DATA/nllb
MODEL=${MODEL:-facebook/nllb-200-distilled-600M}
QUANTIZATION=${QUANTIZATION:-int8}

need python3

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

python3 -m venv "$work/venv"
"$work/venv/bin/pip" install -q --upgrade pip
"$work/venv/bin/pip" install -q ctranslate2 transformers sentencepiece \
    torch --extra-index-url https://download.pytorch.org/whl/cpu

echo "converting $MODEL"
"$work/venv/bin/ct2-transformers-converter" --model "$MODEL" \
    --quantization "$QUANTIZATION" --copy_files tokenizer.json \
    --output_dir "$work/nllb"
mkdir -p "$TARGET"
cp -r "$work/nllb/." "$TARGET/"
echo "NLLB-200 is in $TARGET"
