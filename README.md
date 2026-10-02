# wl-screen-translate

Translates text from the screen under Wayland: select a region, its text is
recognised with Chrome Screen AI, PaddleOCR or Tesseract and translated
offline with Meta NLLB-200.

## Install

Needs Rust, CMake and a C++ compiler.

```sh
cargo install --path .
```

## Models

The program downloads nothing; the scripts put the models into
`~/.local/share/wl-screen-translate/` (`DATA` overrides it).

| script                                     | needs                         |
|--------------------------------------------|-------------------------------|
| `scripts/fetch-screen-ai.sh`               | `curl`, `jq`, `unzip`         |
| `scripts/fetch-screen-ai.sh --from-chrome` | Screen AI in a Chrome profile |
| `scripts/fetch-paddle-ocr.sh`              | `curl`, `tar`, ~130 MB        |
| `scripts/fetch-tesseract.sh`               | `curl`, Tesseract installed   |
| `scripts/fetch-nllb.sh`                    | `python3` with `venv`, ~3 GB  |

One recogniser is enough. Screen AI is built for x86-64 only; PaddleOCR, the
default on ARM, runs anywhere ONNX Runtime does but reads fewer scripts: Latin,
Cyrillic, Greek, Arabic, Devanagari, Tamil, Telugu, Thai, Korean, Chinese and
Japanese. `MODELS` picks the PaddleOCR recognisers fetched, see the script.
Tesseract is the library of the distribution (`dnf install tesseract`, `apt
install libtesseract5`) and reads the languages whose models are installed:
those the script fetched (`LANGUAGES="eng rus"`), else the distribution's
(`tesseract-langpack-rus`, `tesseract-ocr-rus`). English is read with every
other language.

The NLLB-200 weights are licensed CC-BY-NC 4.0, for non-commercial use only.

## Use

Bind `wl-screen-translate` to a shortcut. Drag over text to translate. A right
click drops the region, or quits when there is none; Esc quits. The recogniser
is chosen under Settings in the language window; with PaddleOCR or Tesseract
the languages of the text are those it reads.

## Development

`podman-compose run --rm devclaude` starts Claude Code in a container with the
Rust toolchain; the repository is mounted at `/workspace/ttygui`.
