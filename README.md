# wl-screen-translate

Translates text from the screen under Wayland: select a region, its text is
recognised with Chrome Screen AI and translated offline with Meta NLLB-200.

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
| `scripts/fetch-nllb.sh`                    | `python3` with `venv`, ~3 GB  |

The NLLB-200 weights are licensed CC-BY-NC 4.0, for non-commercial use only.

## Use

Bind `wl-screen-translate` to a shortcut. Drag over text to translate. A right
click drops the region, or quits when there is none; Esc quits.

## Development

`podman-compose run --rm devclaude` starts Claude Code in a container with the
Rust toolchain; the repository is mounted at `/workspace/ttygui`.
