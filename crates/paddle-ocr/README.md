# paddle-ocr

## Scope

Recognises the text of an RGBA image with the PaddleOCR PP-OCRv5 mobile
models, run by ONNX Runtime.

```rust
let mut engine = paddle_ocr::Engine::load(&directory)?;
let lines: Vec<paddle_ocr::Line> = engine.recognize(&image, "Cyrl")?;
```

`directory` holds ONNX Runtime (`RUNTIME_NAME`), the detector (`det.onnx`)
and, in `rec/`, a recogniser `<name>.onnx` with its alphabet `<name>.txt` for
each kind of script; `scripts/fetch-paddle-ocr.sh` puts them there. The script
is an ISO 15924 code; `recognizer_for` names the recogniser that reads it, or
none. `Line` boxes are in pixels of the image given.

## Boundaries

- ONNX Runtime is loaded from the directory at run time, 1.28 or newer. One
  process loads it once.
- Detection follows PaddleOCR's `DBPostProcess` with upright boxes: the map is
  cut into connected regions, which are grown back by the unclip ratio.
  Images are scaled to a longest side between 960 (at most twice) and 2048.
- A line is read alone, scaled to 48 pixels high, by greedy CTC; a line read
  with a mean confidence under 0.5 is dropped.
- The detector knows nothing of paragraphs: `src/layout.rs` puts the lines in
  reading order and groups those just below each other, of a like height and
  overlapping, into a paragraph. A block is a paragraph.
- The engine is not shared between threads at once.

## Errors

| variant    | cause                                                |
|------------|------------------------------------------------------|
| `Runtime`  | ONNX Runtime is not loadable or too old              |
| `Script`   | no recogniser reads the script                       |
| `Missing`  | a model, an alphabet or the runtime is missing       |
| `Alphabet` | an alphabet could not be read                        |
| `Load`     | ONNX Runtime could not load a model                  |
| `Run`      | inference failed                                     |
| `Shape`    | a model answered with an unexpected shape            |
