# nllb

## Scope

Translates texts with an NLLB-200 model converted for CTranslate2.

```rust
let translator = nllb::Translator::load(&directory)?;
let texts: Vec<String> = translator.translate(&texts, "eng_Latn", "rus_Cyrl")?;
let tokens: usize = nllb::Counter::load(&directory)?.count("Hello")?;
```

`directory` holds the converted model and `tokenizer.json` (`TOKENIZER_FILE`).
`LANGUAGES` lists the language codes the model knows; `autonym` names one in
its own script.

## Boundaries

- The source is tokenized without special tokens and framed as
  `[source code] tokens </s>`; the target code is the decoder prefix. This is
  the layout NLLB was trained on, and it lets the source language change per
  call.
- `Counter` loads the tokenizer alone and counts the tokens of a text with
  its frame, as the model gets it.
- A translation may run to twice the longest source, at least 256 and at most
  1024 tokens; CTranslate2 alone stops at 256 and cuts it short.
- The model runs on the CPU through `ct2rs` with the `ruy` backend.
- One call is one batch: the texts are translated together.

## Errors

| variant     | cause                                          |
|-------------|------------------------------------------------|
| `Tokenizer` | `tokenizer.json` is missing or unreadable      |
| `Model`     | the CTranslate2 model is missing or unreadable |
| `Language`  | a language code is not a token of the model    |
| `Encode`    | a text could not be tokenized                  |
| `Translate` | CTranslate2 failed                             |
| `Decode`    | the output tokens could not be joined          |
