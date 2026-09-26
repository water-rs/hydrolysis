use super::*;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// Engine font handles by the identity of the parley font blob they were
/// registered from. Registered once per font face and kept for the
/// renderer's lifetime: glyph runs name fonts by engine id.
#[derive(Default)]
pub(crate) struct FontCache {
    fonts: HashMap<(u64, u32), cherenkov::Font>,
}

impl FontCache {
    pub(crate) fn font(
        &mut self,
        engine: &crate::platform::Engine,
        font: &parley::FontData,
    ) -> cherenkov::FontId {
        let key = (font.data.id(), font.index);
        self.fonts
            .entry(key)
            .or_insert_with(|| {
                engine
                    .font(cherenkov::FontSource {
                        data: Arc::from(font.data.as_ref()),
                        index: font.index,
                    })
                    .expect("hydrolysis: registering a shaped font with the engine failed")
            })
            .id()
    }
}


/// Shared mutable state carried by the hydrolysis dispatcher.
pub struct HydroState {
    /// Thread-safe text shaping/measurement, shared with worker-thread layout
    /// measurement via `Arc`. See [`TextMeasureService`].
    pub(crate) text: Arc<TextMeasureService>,
    pub(crate) measurement: MeasurementCaches,
    /// The engine glyph runs register their fonts with; `None` on a semantic
    /// (non-drawing) runtime, where recording text is a programming error.
    engine: Option<Rc<crate::platform::Engine>>,
    fonts: FontCache,
}

impl Default for HydroState {
    fn default() -> Self {
        Self::new(None)
    }
}

impl HydroState {
    pub(crate) fn new(engine: Option<Rc<crate::platform::Engine>>) -> Self {
        Self {
            text: Arc::new(TextMeasureService::new()),
            measurement: MeasurementCaches::default(),
            engine,
            fonts: FontCache::default(),
        }
    }

    /// The engine font id for a shaped parley font, registering it on first
    /// use.
    ///
    /// # Panics
    /// Panics on a runtime without an engine: only a drawing runtime records
    /// glyph runs.
    pub(crate) fn font_id(&mut self, font: &parley::FontData) -> cherenkov::FontId {
        let engine = self
            .engine
            .as_ref()
            .expect("hydrolysis: recording text requires a Cherenkov engine; this runtime has none");
        self.fonts.font(engine, font)
    }

    /// Mutable access to the registered fonts for startup font registration.
    ///
    /// Requires that no worker has cloned the [`TextMeasureService`] yet, which
    /// holds during single-threaded setup before the first render/measure.
    pub(crate) fn text_fonts_mut(&mut self) -> &mut parley::FontContext {
        Arc::get_mut(&mut self.text)
            .expect(
                "hydrolysis font registration requires unique TextMeasureService ownership \
                 before rendering",
            )
            .fonts_mut()
    }
}

impl core::fmt::Debug for HydroState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HydroState").finish_non_exhaustive()
    }
}
