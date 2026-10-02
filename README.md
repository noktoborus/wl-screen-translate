# wl-screen-translate

Translates text from the screen under Wayland: select a region, its text is
recognised with Chrome Screen AI, PaddleOCR or Tesseract and translated
offline with Meta NLLB-200.

## Install

The releases hold the program for Linux x86-64 (glibc 2.35 or newer), Linux
ARM64 and Windows x86-64, with the scripts that download the models; the
models are not in them. Unpack one, then fetch the models below.

To build it instead, Rust, CMake and a C++ compiler are needed:

```sh
cargo install --path .
```

A tag `v*` pushed to GitHub builds the three archives and makes a release of
them (`.github/workflows/release.yml`); a run started by hand keeps them as
artifacts of the run.

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

Under Windows the scripts are PowerShell ones of the same names, `.ps1`, and
the models go into `%APPDATA%\wl-screen-translate\data\`:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\fetch-paddle-ocr.ps1
```

`fetch-screen-ai.ps1 -FromChrome` copies Screen AI from Chrome; Tesseract
comes from the installer of UB Mannheim, in its default folder, and
`fetch-nllb.ps1` needs Python 3.

The NLLB-200 weights are licensed CC-BY-NC 4.0, for non-commercial use only.

## Use

Bind `wl-screen-translate` to a shortcut. Drag over text to translate. A right
click drops the region, or quits when there is none; Esc quits. The recogniser
is chosen under Settings in the language window; with PaddleOCR or Tesseract
the languages of the text are those it reads.

## Development

`podman-compose run --rm devclaude` starts Claude Code in a container with the
Rust toolchain; the repository is mounted at `/workspace/ttygui`.
