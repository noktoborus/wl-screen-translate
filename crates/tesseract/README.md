# tesseract

## Scope

Recognises the text of an RGBA image with the Tesseract library of the
system, loaded at run time through its C API.

```rust
let directory = tesseract::model_directory(&preferred).unwrap();
let languages: Vec<String> = tesseract::languages(&directory);
let mut engine = tesseract::Engine::load(&directory)?;
let lines: Vec<tesseract::Line> = engine.recognize(&image, "rus+eng")?;
```

`model_directory` takes `preferred` if it holds a `*.traineddata` model, else
the directory of `TESSDATA_PREFIX` or of the distribution. `languages` lists
its models but `osd` and `equ`. `Line` boxes are in pixels of the image given.

## Boundaries

- The library is looked for as `LIBRARY_NAMES` say (`libtesseract.so.5`
  first); the calls are those of `tesseract/capi.h`, Tesseract 4 or 5.
- The handle is initialised for one set of models at a time, `eng+rus` for
  two; changing it reads the models again, a fraction of a second.
- The image is made grey, inverted when its mean is dark (light text on a
  dark ground), and enlarged 2.5 times, at most to 8000 pixels, and said to be
  300 dpi: Tesseract reads print, screen letters are a dozen pixels high.
- The page is segmented automatically (`PSM_AUTO`); blocks and paragraphs are
  Tesseract's own. A line read with a confidence under 30 is dropped.
- The engine is not shared between threads at once.

## Errors

| variant     | cause                                              |
|-------------|----------------------------------------------------|
| `Load`      | no library name is loadable                        |
| `Symbol`    | the library lacks a function this crate calls      |
| `Missing`   | the model of a language is not in the directory    |
| `Init`      | `TessBaseAPIInit3` failed                          |
| `Recognize` | `TessBaseAPIRecognize` failed                      |
