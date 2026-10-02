# screen-ai

## Scope

Loads the Chrome Screen AI library and recognises the text of an RGBA image.

```rust
let engine = screen_ai::Engine::load(&directory)?;
let lines: Vec<screen_ai::Line> = engine.recognize(&image)?;
```

`directory` is the component directory: the library (`LIBRARY_NAME`) and its
model folders. `Line` boxes are in pixels of the image given.

## Boundaries

- One process loads from one directory. The library keeps global state and
  reads its models through process-wide callbacks.
- The library has no public interface. The calls follow Chromium's
  `services/screen_ai/screen_ai_library_wrapper_impl.cc`.
- `PerformOCR` reads a Skia `SkBitmap` by its compiled layout, built in
  `src/bitmap.rs`. A library built against another Skia may need a new layout;
  the test there pins the offsets in use.
- An image larger than `GetMaxImageDimension` is scaled down before
  recognition; the boxes are scaled back.
- The result is the `chrome_screen_ai.VisualAnnotation` protobuf, reduced in
  `src/proto.rs` to the fields read.
- The engine is not shared between threads at once.

## Errors

| variant          | cause                                             |
|------------------|---------------------------------------------------|
| `Load`           | the library file is missing or not loadable       |
| `Symbol`         | the library lacks a function this crate calls     |
| `OtherDirectory` | an engine was loaded from another directory       |
| `Init`           | initialisation failed, usually a missing model    |
| `Recognize`      | `PerformOCR` returned nothing                     |
| `Decode`         | the result is not a `VisualAnnotation`            |
