#!/bin/sh
# Downloads Tesseract language models into $DATA/tesseract. The library comes
# from the distribution: dnf install tesseract, apt install libtesseract5.
# Without models here, the program reads those of the distribution, such as
# tesseract-langpack-rus or tesseract-ocr-rus.
# LANGUAGES picks the models, by Tesseract's names (eng rus deu chi_sim ...);
# English is read with every other language. MODELS picks fast or best:
# best reads a little better, several times slower.
set -eu
. "$(dirname "$0")/common.sh"

TARGET=$DATA/tesseract
LANGUAGES=${LANGUAGES:-eng rus}
MODELS=${MODELS:-fast}

case $MODELS in
    fast | best) ;;
    *) die "MODELS is fast or best, not $MODELS" ;;
esac

need curl

if command -v ldconfig >/dev/null && ! ldconfig -p | grep -q libtesseract.so; then
    echo "$0: libtesseract is not installed; install Tesseract of the distribution" >&2
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

for language in $LANGUAGES; do
    echo "downloading $language"
    curl -fL --progress-bar \
        "https://raw.githubusercontent.com/tesseract-ocr/tessdata_$MODELS/main/$language.traineddata" \
        -o "$work/$language.traineddata"
done
mkdir -p "$TARGET"
cp "$work"/*.traineddata "$TARGET/"
echo "Tesseract models are in $TARGET"
