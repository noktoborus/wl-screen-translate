//! The thread that recognises and translates, so the interface never waits.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use eframe::egui::{Context, Pos2};
use image::RgbaImage;

use crate::cache::{Cache, Entry};
use crate::error::{AppError, Result};
use crate::ocr::{Ocr, Recognizers};
use crate::paragraph::{self, Paragraph};
use crate::paths::Paths;
use crate::split::{self, Split, Unit};

/// The languages of one translation.
#[derive(Debug, Clone)]
pub struct Languages {
    /// The language of the text.
    pub source: String,
    /// The language to translate to.
    pub target: String,
}

/// A job for the worker. `id` names the job in its responses.
pub enum Request {
    /// Recognise a region, then translate it.
    Recognize {
        /// The job.
        id: u64,
        /// The region, cut from the screenshot.
        image: RgbaImage,
        /// Where the region starts in the screenshot.
        origin: Pos2,
        /// The recogniser.
        ocr: Ocr,
        /// How the text is cut for translation.
        split: Split,
        /// The most tokens of a piece, if they are limited.
        token_limit: Option<usize>,
        /// The languages to translate with.
        languages: Languages,
    },
    /// Translate the pieces of a text already recognised.
    Translate {
        /// The job.
        id: u64,
        /// The plates and their pieces, as [`split::units`] cuts them.
        units: Vec<Unit>,
        /// The most tokens of a piece, if they are limited.
        token_limit: Option<usize>,
        /// The languages to translate with.
        languages: Languages,
    },
}

impl Request {
    /// The job.
    fn id(&self) -> u64 {
        match self {
            Request::Recognize { id, .. } | Request::Translate { id, .. } => *id,
        }
    }
}

/// An answer of the worker.
pub enum Response {
    /// The recognised paragraphs.
    Recognized {
        /// The job.
        id: u64,
        /// The paragraphs.
        paragraphs: Vec<Paragraph>,
        /// How long recognition took, with loading the recogniser.
        elapsed: Duration,
        /// A translation follows: translating was on after recognition.
        translating: bool,
    },
    /// The plates and their pieces cut again by the token limit, before
    /// any translation of the job: the pieces translated are these.
    Cut {
        /// The job.
        id: u64,
        /// The plates and their pieces.
        units: Vec<Unit>,
        /// How many pieces more the limit made.
        cuts: usize,
    },
    /// The translation of one piece; the pieces come as they are translated,
    /// those of the cache first.
    Translated {
        /// The job.
        id: u64,
        /// The piece, in the order of the pieces.
        index: usize,
        /// Its translation.
        text: String,
    },
    /// Every piece is translated.
    Done {
        /// The job.
        id: u64,
        /// How long translation took, with loading the model.
        elapsed: Duration,
    },
    /// The job failed.
    Failed {
        /// The job.
        id: u64,
        /// The failure.
        error: AppError,
    },
}

/// The handle of the worker thread.
pub struct Worker {
    requests: Sender<Request>,
    responses: Receiver<Response>,
    /// The job the interface waits for, [`NO_JOB`] when none: any other is
    /// dropped at its next stage.
    current: Arc<AtomicU64>,
    /// Whether a recognised text is translated.
    translate: Arc<AtomicBool>,
}

/// The value of [`Worker::current`] when every job is cancelled.
const NO_JOB: u64 = 0;

impl Worker {
    /// Starts the thread; it wakes the interface after every response. A
    /// recognised text is translated when `translate` is on.
    pub fn spawn(paths: Paths, context: Context, translate: bool) -> Self {
        let (requests, jobs) = channel();
        let (answers, responses) = channel();
        let current = Arc::new(AtomicU64::new(NO_JOB));
        let worker_current = Arc::clone(&current);
        let translate = Arc::new(AtomicBool::new(translate));
        let worker_translate = Arc::clone(&translate);
        std::thread::spawn(move || {
            let mut engines = Engines {
                cache: Cache::new(paths.translations.clone()),
                current: worker_current,
                translate: worker_translate,
                paths,
                recognizers: Recognizers::default(),
                translator: None,
                counter: None,
            };
            for job in jobs {
                engines.run(job, &|response| {
                    let _ = answers.send(response);
                    context.request_repaint();
                });
            }
        });
        Self {
            requests,
            responses,
            current,
            translate,
        }
    }

    /// Turns the translation of recognised texts on or off; a recognition
    /// under way follows it.
    pub fn set_translate(&self, translate: bool) {
        self.translate.store(translate, Ordering::Relaxed);
    }

    /// Queues a job, which cancels the others.
    pub fn send(&self, request: Request) {
        self.current.store(request.id(), Ordering::Relaxed);
        let _ = self.requests.send(request);
    }

    /// Cancels every job: each stops at its next stage. Recognition or
    /// translation under way is not interrupted, its result is dropped.
    pub fn cancel(&self) {
        self.current.store(NO_JOB, Ordering::Relaxed);
    }

    /// The next response, if one is ready.
    pub fn poll(&self) -> Option<Response> {
        self.responses.try_recv().ok()
    }
}

/// The engines, loaded on first use because loading takes seconds.
struct Engines {
    paths: Paths,
    cache: Cache,
    current: Arc<AtomicU64>,
    translate: Arc<AtomicBool>,
    recognizers: Recognizers,
    translator: Option<nllb::Translator>,
    /// The tokenizer alone, to cut pieces by the token limit.
    counter: Option<nllb::Counter>,
}

impl Engines {
    fn run(&mut self, job: Request, answer: &dyn Fn(Response)) {
        if self.cancelled(job.id(), "before it started") {
            return;
        }
        let (id, units, token_limit, languages) = match job {
            Request::Recognize {
                id,
                image,
                origin,
                ocr,
                split,
                token_limit,
                languages,
            } => {
                let started = Instant::now();
                match self.recognize(ocr, &image, origin, &languages.source) {
                    Ok(paragraphs) => {
                        let units = split::units(&paragraphs, split);
                        let translating = self.translate.load(Ordering::Relaxed);
                        answer(Response::Recognized {
                            id,
                            paragraphs,
                            elapsed: started.elapsed(),
                            translating,
                        });
                        if !translating || self.cancelled(id, "after recognition") {
                            return;
                        }
                        (id, units, token_limit, languages)
                    }
                    Err(error) => return answer(Response::Failed { id, error }),
                }
            }
            Request::Translate {
                id,
                units,
                token_limit,
                languages,
            } => (id, units, token_limit, languages),
        };
        let units = match token_limit {
            Some(max) => match self.limit(units, max) {
                Ok((units, cuts)) => {
                    answer(Response::Cut {
                        id,
                        units: units.clone(),
                        cuts,
                    });
                    units
                }
                Err(error) => return answer(Response::Failed { id, error }),
            },
            None => units,
        };
        let pieces: Vec<String> = units.into_iter().flat_map(|unit| unit.pieces).collect();
        let started = Instant::now();
        match self.translate(id, &pieces, &languages, answer) {
            Ok(true) => answer(Response::Done {
                id,
                elapsed: started.elapsed(),
            }),
            Ok(false) => {}
            Err(error) => answer(Response::Failed { id, error }),
        }
    }

    /// Whether job `id` was cancelled; it is logged as stopped at `stage`.
    fn cancelled(&self, id: u64, stage: &str) -> bool {
        let cancelled = self.current.load(Ordering::Relaxed) != id;
        if cancelled {
            log::info!("job {id} cancelled {stage}");
        }
        cancelled
    }

    /// Recognises `image`, cut at `origin`, written in `language`, with
    /// `ocr`.
    fn recognize(
        &mut self,
        ocr: Ocr,
        image: &RgbaImage,
        origin: Pos2,
        language: &str,
    ) -> Result<Vec<Paragraph>> {
        log::info!(
            "recognising {}x{} pixels at {:?} with {}",
            image.width(),
            image.height(),
            origin,
            ocr.name()
        );
        let started = Instant::now();
        let lines = self
            .recognizers
            .recognize(ocr, &self.paths, image, language)?;
        let paragraphs = paragraph::group(lines, origin);
        log::info!(
            "recognised {} paragraphs in {:?}",
            paragraphs.len(),
            started.elapsed()
        );
        Ok(paragraphs)
    }

    /// Cuts each piece of `units` longer than `max` tokens, and counts the
    /// pieces it made more. The tokenizer is loaded on first use, without
    /// the model.
    fn limit(&mut self, units: Vec<Unit>, max: usize) -> Result<(Vec<Unit>, usize)> {
        let counter = match &mut self.counter {
            Some(counter) => counter,
            empty => {
                let started = Instant::now();
                let counter = nllb::Counter::load(&self.paths.nllb)
                    .map_err(|source| AppError::TranslatorLoad { source })?;
                log::info!("NLLB tokenizer loaded in {:?}", started.elapsed());
                empty.insert(counter)
            }
        };
        let mut cuts = 0;
        let units = units
            .into_iter()
            .map(|unit| {
                let mut pieces = Vec::with_capacity(unit.pieces.len());
                for piece in &unit.pieces {
                    let chunks = split::limit(piece, max, |text| counter.count(text))
                        .map_err(|source| AppError::Translate { source })?;
                    cuts += chunks.len() - 1;
                    pieces.extend(chunks);
                }
                Ok(Unit { pieces, ..unit })
            })
            .collect::<Result<Vec<_>>>()?;
        if cuts > 0 {
            log::info!("{cuts} pieces more by the limit of {max} tokens");
        }
        Ok((units, cuts))
    }

    /// Answers the pieces in the cache, then translates the others one by
    /// one, answering and caching each, so the model is not loaded when every
    /// piece is cached. False when job `id` is cancelled between two pieces.
    fn translate(
        &mut self,
        id: u64,
        pieces: &[String],
        languages: &Languages,
        answer: &dyn Fn(Response),
    ) -> Result<bool> {
        let mut missing = Vec::new();
        for (index, piece) in pieces.iter().enumerate() {
            let entry = self.cache.entry(languages, piece);
            match entry.as_ref().and_then(Entry::get) {
                Some(text) => answer(Response::Translated { id, index, text }),
                None => missing.push((index, entry)),
            }
        }
        log::info!(
            "{} of {} pieces from the cache",
            pieces.len() - missing.len(),
            pieces.len()
        );
        for (index, entry) in missing {
            if self.cancelled(id, "before translation") {
                return Ok(false);
            }
            let piece = std::slice::from_ref(&pieces[index]);
            let text = self
                .translate_with_model(piece, languages)?
                .pop()
                .unwrap_or_default();
            if let Some(entry) = entry {
                entry.put(&text);
            }
            answer(Response::Translated { id, index, text });
        }
        Ok(true)
    }

    fn translate_with_model(
        &mut self,
        texts: &[String],
        languages: &Languages,
    ) -> Result<Vec<String>> {
        let translator = match &mut self.translator {
            Some(translator) => translator,
            empty => {
                log::info!("loading NLLB from {}", self.paths.nllb.display());
                let started = Instant::now();
                let translator = nllb::Translator::load(&self.paths.nllb)
                    .map_err(|source| AppError::TranslatorLoad { source })?;
                log::info!("NLLB loaded in {:?}", started.elapsed());
                empty.insert(translator)
            }
        };
        log::info!(
            "translating {} pieces from {} to {}",
            texts.len(),
            languages.source,
            languages.target
        );
        let started = Instant::now();
        let translated = translator
            .translate(texts, &languages.source, &languages.target)
            .map_err(|source| AppError::Translate { source })?;
        log::info!("translated in {:?}", started.elapsed());
        Ok(translated)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    fn engines(current: u64, dir: &std::path::Path) -> Engines {
        Engines {
            paths: Paths {
                settings: dir.join("settings.yaml"),
                screen_ai: dir.join("screen-ai"),
                paddle_ocr: dir.join("paddle-ocr"),
                tesseract: dir.join("tesseract"),
                nllb: dir.join("nllb"),
                translations: dir.join("translations"),
            },
            cache: Cache::new(dir.join("translations")),
            current: Arc::new(AtomicU64::new(current)),
            translate: Arc::new(AtomicBool::new(true)),
            recognizers: Recognizers::default(),
            translator: None,
            counter: None,
        }
    }

    fn translate(id: u64) -> Request {
        Request::Translate {
            id,
            units: split::units(
                &[Paragraph {
                    text: "Hello".into(),
                    rect: eframe::egui::Rect::NOTHING,
                    line_height: 10.0,
                }],
                Split::Sentences,
            ),
            token_limit: None,
            languages: Languages {
                source: "eng_Latn".into(),
                target: "rus_Cyrl".into(),
            },
        }
    }

    /// The ids of the jobs answered, `true` for a translation.
    fn answers(engines: &mut Engines, job: Request) -> Vec<(u64, bool)> {
        let answers = RefCell::new(Vec::new());
        engines.run(job, &|response| {
            answers.borrow_mut().push(match response {
                Response::Translated { id, .. } => (id, true),
                Response::Recognized { id, .. }
                | Response::Cut { id, .. }
                | Response::Done { id, .. }
                | Response::Failed { id, .. } => (id, false),
            })
        });
        answers.into_inner()
    }

    #[test]
    fn a_cancelled_job_is_not_answered() {
        let dir = tempfile::tempdir().unwrap();
        let mut engines = engines(2, dir.path());
        assert_eq!(answers(&mut engines, translate(1)), []);
        engines.current.store(NO_JOB, Ordering::Relaxed);
        assert_eq!(answers(&mut engines, translate(2)), []);
    }

    #[test]
    fn the_current_job_is_answered() {
        let dir = tempfile::tempdir().unwrap();
        let mut engines = engines(3, dir.path());
        let Request::Translate { languages, .. } = translate(3) else {
            unreachable!()
        };
        engines
            .cache
            .entry(&languages, "Hello")
            .unwrap()
            .put("Привет");
        assert_eq!(answers(&mut engines, translate(3)), [(3, true), (3, false)]);
    }
}
