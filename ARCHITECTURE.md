# Architecture

## Crates

| crate                | purpose                                              |
|----------------------|------------------------------------------------------|
| `wl-screen-translate`| the binary: capture, interface, settings, worker      |
| `crates/screen-ai`   | text recognition through the Screen AI library        |
| `crates/paddle-ocr`  | text recognition with PaddleOCR through ONNX Runtime  |
| `crates/tesseract`   | text recognition through the system's Tesseract       |
| `crates/nllb`        | translation with a CTranslate2 NLLB-200 model         |

The library crates know nothing of each other, of egui or of the settings.
Their surfaces are the `README.md` of each.

## Threads

| thread    | owns                                   | blocks on               |
|-----------|----------------------------------------|-------------------------|
| main      | the screenshot, the window, settings   | nothing after start     |
| worker    | the recognisers, `nllb::Translator`    | recognition, translation|
| fonts     | fontconfig, the font files mapped       | a letter without a font |

The main thread captures the screen before the window opens, so the window is
not in the screenshot. Every engine loads on its first job: loading takes
seconds and the window should not wait for it.

## Data path

```
portal ──png──► RgbaImage ──crop per monitor──► textures (egui), a window each
                    │
          region ───┴─crop─► Request::Recognize ──► worker
                                                     │
            Response::Recognized(paragraphs) ◄───────┤ ocr::Recognizers::recognize
            Response::Translated(piece) …    ◄───────┤ nllb::Translator::translate
            Response::Done                   ◄───────┘
```

`src/worker.rs` carries jobs and answers over two `std::sync::mpsc` channels.
Each job has an id; the interface drops answers to a job it has replaced.
A region dragged while a job runs replaces it. The worker shares the id of
the job awaited and stops any other before it starts, after recognition and
before the model translates each piece; a recognition or the translation of
a piece under way runs to its end (neither library can be interrupted, short
of greedy decoding) and its result is dropped.

When the translate switch of the language window is off, the worker stops
after recognition: the cache is not looked in and the model is not run. The
switch is an atomic flag read after recognition, so a recognition under way
follows it; `Recognized` tells whether a translation follows.

The text is translated in pieces, cut by `src/split.rs` as the split button
of the language window says: the whole text (one plate over the region), each
paragraph, or each sentence of a paragraph (Unicode sentence bounds). The
interface and the worker cut the paragraphs with the same function; a
`Translate` request carries the plates and their pieces.

When the token limit of the Settings is on, the worker cuts again each piece
longer than the limit: at the word boundary nearest it, so a word goes into
this piece or the next, and between the letters of a word longer than the
limit alone. The tokens are counted by the NLLB tokenizer, loaded alone
(`nllb::Counter`), as the model gets them. Each part is a piece of its own,
translated and cached alone; `Cut` gives the interface the pieces before the
first translation, with how many the limit added, shown in the statistics.

Before translating, the worker looks each piece up in the cache
(`src/cache.rs`): a file `<source>-<target>/<XXH3-128 of the text>` in the
user's runtime directory (`$XDG_RUNTIME_DIR`, else the temporary directory).
It answers those found at once, then gives the others to the model one by
one, answering and caching each, so the plates fill as the translation goes;
the model is not loaded when every piece is cached. `Done` ends the job with
its time, shown with the other statistics under the buttons.

`src/ocr.rs` holds the recognisers: Screen AI, PaddleOCR and Tesseract,
picked by the settings (`Ocr`, PaddleOCR by default on ARM, where Screen AI
has no build). Each `Recognize` request names the one to use; each is loaded
on first use and kept. PaddleOCR reads a script with a model of its own,
taken from the language of the text; Tesseract reads a language with its own
model and English. Both read fewer languages than NLLB translates: the menu
of the language of the text and the recent directions list only those they
read, and a language they do not read is replaced by the default.
`src/tessdata.rs` finds the Tesseract models once at start, in the data
directory or else the system's, and names the model of each NLLB code. A
change of recogniser, or of the language of the text to one read with another
model, recognises the region again.

The recogniser returns lines. `src/paragraph.rs` joins them by block and
paragraph, because a sentence split over lines translates badly line by line.
Screen AI and Tesseract find the paragraphs themselves; PaddleOCR finds lines
only, and its crate groups them by their layout.

The portal returns every monitor in one image. On the first frame
`src/monitor.rs` finds each monitor in the screenshot, laid out as the
compositor reports it through xdg-output (`src/outputs.rs`; winit gives the
video mode, which ignores rotation and fractional scale), and every
monitor gets a fullscreen window (an immediate egui viewport, the root one on
monitor 0) showing its own part. A region is dragged on one of them; its text
is drawn there. The language window is drawn on the monitor the pointer was
last over; a language is picked from a `plate-menu` (a crate of zyterm) opened
over that monitor. Under its Settings header are the split and the
recogniser buttons, each opening its menu, and the statistics. Its copy button
copies the translated text, or the recognised one when translation is off, and
quits. Under Linux the copy starts the
program again with `--serve-clipboard` (`src/clipboard.rs`): this background
process sets the text with arboard, through data-control or, under GNOME,
which has none, the X11 clipboard of XWayland, and serves it until something
else is copied, so it can be pasted after the program quits. When the monitors do not match the image, the whole
screenshot is shown in the root window.

## Fonts

egui starts with its own fonts; nothing is looked for at start. When a
paragraph or a translation arrives, `src/fonts.rs` checks its letters against
the fonts of egui and sends those no font has, each once, to the font thread.
It loads libfontconfig (`src/fontconfig.rs`) on the first such letter and asks
for a sans-serif face with it: fontconfig answers from the cache the system
keeps, without reading a font file. A face found strikes off every other
letter it has, so a script takes one face. Its file is memory-mapped and given
to egui as the lowest fallback; only the pages of glyphs drawn are loaded.

## States of the window

```
Selecting ──drag stopped──► Busy ──Recognized──► Busy (text shown) ◄─Translated─┐
    ▲                                              │       └────────────────────┘
    └──────── drag started ◄── Shown ◄────Done─────┘
                                 │
       language or split changed └──► Busy ──Translated…, Done──► Shown
```

## Errors

Each crate has one `Error` enum in `error.rs`. `src/error.rs` wraps them in
`AppError`, whose `message_key` selects the user text in `locales/app.yml`.
A worker error is shown over the screenshot; a start-up error goes to stderr.
