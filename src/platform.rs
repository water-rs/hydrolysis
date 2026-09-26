use std::rc::Rc;

use nami::Signal;
use waterui::cursor::CursorStyle;
use waterui::window::{Window as WuiWindow, WindowState};

/// Input button mapped from a platform pointer event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
    Other(u16),
}

/// Physical pointer source reported by the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Mouse,
    Touch,
    Pen,
}

/// Input key state mapped from a platform keyboard event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

/// Platform-agnostic key identifier.
///
/// This is the vocabulary `WaterUI`'s own widgets and the embedded browser
/// bridges match on. New code should read [`InputEvent::Key`]'s `logical_key`
/// and `physical_code` instead — the W3C UI Events pair, which is what GPU
/// surfaces receive and what the browser engines will move to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyCode {
    Character(String),
    Named(String),
    Unidentified,
}

impl KeyCode {
    /// The W3C UI Events logical key this identifier denotes.
    ///
    /// A producer holding the platform's own key event maps that directly into
    /// [`InputEvent::Key`]'s `logical_key`, which is strictly better. This is
    /// for the synthetic keystrokes a test driver injects, where the
    /// identifier is the only thing there is.
    #[must_use]
    pub fn to_w3c_key(&self) -> keyboard_types::Key {
        let unidentified = keyboard_types::Key::Named(keyboard_types::NamedKey::Unidentified);
        match self {
            Self::Character(value) => keyboard_types::Key::Character(value.clone()),
            // The W3C vocabulary has no named "Space": it is the character the
            // key types.
            Self::Named(value) if value == "Space" => {
                keyboard_types::Key::Character(" ".to_owned())
            }
            Self::Named(value) => value.parse().unwrap_or(unidentified),
            Self::Unidentified => unidentified,
        }
    }
}

/// Active key modifiers snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub super_key: bool,
}

impl From<Modifiers> for keyboard_types::Modifiers {
    fn from(modifiers: Modifiers) -> Self {
        let mut result = Self::empty();
        result.set(Self::SHIFT, modifiers.shift);
        result.set(Self::CONTROL, modifiers.control);
        result.set(Self::ALT, modifiers.alt);
        result.set(Self::META, modifiers.super_key);
        result
    }
}

/// IME purpose for the focused text input target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputPurpose {
    Normal,
    Password,
}

pub use waterui_backend_core::input::TouchPhase;

/// Focused text-input area used for IME activation and candidate-window placement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextInputState {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub purpose: TextInputPurpose,
}

/// Input events emitted by a windowing backend.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    PointerDown {
        id: u64,
        kind: PointerKind,
        x: f32,
        y: f32,
        button: PointerButton,
    },
    PointerUp {
        id: u64,
        kind: PointerKind,
        x: f32,
        y: f32,
        button: PointerButton,
    },
    PointerMove {
        id: u64,
        kind: PointerKind,
        x: f32,
        y: f32,
    },
    PointerCancel {
        id: u64,
        kind: PointerKind,
    },
    Moved {
        x: f32,
        y: f32,
    },
    Scroll {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        is_line_delta: bool,
    },
    TrackpadPan {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        phase: TouchPhase,
    },
    Magnification {
        x: f32,
        y: f32,
        delta: f32,
        phase: TouchPhase,
    },
    Rotation {
        x: f32,
        y: f32,
        delta: f32,
        phase: TouchPhase,
    },
    TextInput {
        text: String,
    },
    Key {
        key: KeyCode,
        /// The logical key in the W3C UI Events vocabulary — what the layout
        /// and modifiers produce. Unlike `key`, this is never suppressed when
        /// the same keystroke also produces text: an embedded engine needs the
        /// real `keydown` alongside the insertion, exactly as the web does.
        logical_key: keyboard_types::Key,
        /// The physical key in the W3C UI Events vocabulary — where it sits on
        /// the keyboard, independent of layout.
        physical_code: keyboard_types::Code,
        /// Whether the platform generated this press by auto-repeat.
        repeat: bool,
        state: KeyState,
        modifiers: Modifiers,
    },
    /// A focus-change replay released a key that was held: winit resends
    /// every held key as a synthetic release when the window loses focus
    /// (on X11, at `XI_FocusOut`). The press it belonged to is aborted, not
    /// completed — the armed keyboard activation and its pressed affordance
    /// come down without firing an action, so a real release arriving later
    /// finds nothing stale left to activate.
    KeyboardCancel,
    ModifiersChanged(Modifiers),
    ImePreedit {
        text: String,
        /// Caret offset within `text`, in bytes, when the platform reports one.
        caret: Option<usize>,
    },
    ImeCommit {
        text: String,
    },
    ImeDisabled,
    Resize {
        width: u32,
        height: u32,
    },
    /// The OS window gained (`true`) or lost (`false`) focus.
    ///
    /// This is the window's own activation, not a focus move inside it:
    /// keyboard focus stays where it was, and the surface holding it is
    /// told focus left and returned so it can report the transition (a
    /// terminal's DECSET 1004 focus tracking, for one).
    Focused(bool),
    CloseRequested,
}

/// Errors raised by a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceError {
    /// The engine's render thread is gone.
    Lost,
}

impl core::fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Lost => "surface was lost",
        })
    }
}

impl std::error::Error for SurfaceError {}

impl From<cherenkov::SurfaceError> for SurfaceError {
    fn from(error: cherenkov::SurfaceError) -> Self {
        match error {
            cherenkov::SurfaceError::Lost => Self::Lost,
            other => panic!("hydrolysis surface: {other}"),
        }
    }
}

/// The Cherenkov engine every surface of a runtime draws on.
pub type Engine = cherenkov::Engine<cherenkov_gpu::Gpu>;

/// Rendering surface abstraction consumed by hydrolysis runner/renderer: a
/// Cherenkov surface plus the engine it belongs to.
pub trait SurfaceProvider {
    /// The engine the surface was created from.
    fn engine(&self) -> &Rc<Engine>;
    /// The surface itself: its root layer takes the frame's content.
    fn surface(&self) -> &cherenkov::Surface<cherenkov_gpu::Gpu>;
    /// The drawable size in physical pixels.
    fn size(&self) -> (u32, u32);
    /// Resizes the drawable.
    fn resize(&mut self, width: u32, height: u32);
}

/// Asserts the app's `Window::frame` binding carries finite components on
/// all four fields. The binding is a trust boundary — a NaN or infinite
/// frame is a programming error, not something the runner silently repairs
/// deeper in the geometry path.
pub(crate) fn validated_window_frame(
    frame: waterui_core::layout::Rect,
) -> waterui_core::layout::Rect {
    for (field, value) in [
        ("x", frame.x()),
        ("y", frame.y()),
        ("width", frame.width()),
        ("height", frame.height()),
    ] {
        assert!(
            value.is_finite(),
            "hydrolysis runner: Window::frame.{field} must be finite, got {value}"
        );
    }
    frame
}

/// Window abstraction consumed by hydrolysis runner.
pub trait PlatformWindow: 'static {
    fn surface(&mut self) -> &mut dyn SurfaceProvider;
    fn apply_properties(&mut self, window: &WuiWindow);
    /// Applies the window's effective content-size limits (logical units).
    ///
    /// The minimum is the content's layout minimum, overridden by an explicit
    /// `Window::min_size`. The maximum stays `None` — resizable and
    /// maximizable — unless the app pins `Window::max_size`: content never
    /// contributes a maximum, since content that does not stretch on an axis
    /// is laid out inside a larger offer per the layout spec rather than
    /// capping the window. A `+∞` component inside an explicit `Some` max is
    /// the app's per-axis unbounded — treat it as "no bound on that axis".
    /// Targets without per-window runtime size limits
    /// (offscreen surfaces, web canvases, fixed embedded displays) keep this
    /// default no-op.
    fn set_size_limits(
        &mut self,
        min: Option<waterui_core::layout::Size>,
        max: Option<waterui_core::layout::Size>,
    ) {
        let _ = (min, max);
    }
    /// Whether this window acts on content-derived size limits.
    ///
    /// Deriving them costs four extra whole-tree measure passes per frame, so a
    /// surface that cannot resize to fit its content — offscreen capture, an
    /// embedded GPU host, a fixed-size shell — leaves this `false` and never pays
    /// for them. Defaults to `false` alongside the no-op [`Self::set_size_limits`].
    fn applies_size_limits(&self) -> bool {
        false
    }
    fn drain_events(&mut self) -> Vec<InputEvent>;
    fn request_redraw(&self);
    fn scale_factor(&self) -> f64;
    /// The refresh rate (Hz) of the display this window is on, if known.
    ///
    /// Drives the game-engine continuous-render frame budget and the diagnostics
    /// slow-frame threshold. Returns `None` on headless/offscreen/web paths with no
    /// monitor information, where the renderer falls back to its default pacing.
    fn refresh_rate_hz(&self) -> Option<f64> {
        None
    }
    fn sync_text_input_state(&mut self, state: Option<TextInputState>);
    fn set_cursor_style(&mut self, style: CursorStyle);
}

/// The engine an [`OffscreenSurface`] renders on.
///
/// A Cherenkov engine owns a wgpu device — a heavyweight, driver-allocated
/// resource, heavyweight in *system* memory too on a machine whose only
/// adapter is a software rasterizer. A caller that builds many surfaces
/// creates one context and hands a clone to every surface: the engine is
/// shared, everything a measurement is about — the view tree, the renderer,
/// the retained scene — is still built fresh per surface.
#[derive(Clone, Debug)]
pub struct OffscreenGpuContext {
    engine: Rc<Engine>,
}

impl OffscreenGpuContext {
    /// Creates an engine on the adapter WaterUI would render an application on.
    ///
    /// # Panics
    /// Panics when no suitable wgpu adapter exists: Hydrolysis requires a GPU.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(gpu_config("hydrolysis offscreen surface"))
    }

    /// Creates an engine for WaterUI test hosts. Software adapters (llvmpipe,
    /// lavapipe) are eligible so CI runs the renderer without a GPU.
    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn new_for_tests() -> Self {
        Self::new()
    }

    /// [`Self::new_for_tests`], for synchronous test harnesses.
    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn new_for_tests_blocking() -> Self {
        Self::new_for_tests()
    }

    /// Creates an engine from an explicit configuration.
    ///
    /// # Panics
    /// Panics when the engine cannot initialize.
    #[must_use]
    pub fn with_config(config: cherenkov_gpu::GpuConfig) -> Self {
        let engine = Engine::new(config).unwrap_or_else(|error| {
            panic!(
                "hydrolysis offscreen surface: failed to create the Cherenkov engine: {error}. \
{GPU_REQUIRED_GUIDANCE}"
            )
        });
        log_selected_adapter("hydrolysis offscreen surface", &engine);
        Self {
            engine: Rc::new(engine),
        }
    }

    /// Wraps an existing engine.
    #[must_use]
    pub fn from_engine(engine: Rc<Engine>) -> Self {
        Self { engine }
    }

    /// The engine.
    #[must_use]
    pub fn engine(&self) -> &Rc<Engine> {
        &self.engine
    }

    /// Lets the engine release everything dropped since the last call.
    pub fn reclaim(&self) {
        self.engine.trim(cherenkov::Pressure::Moderate);
    }
}

impl Default for OffscreenGpuContext {
    fn default() -> Self {
        Self::new()
    }
}

const GPU_REQUIRED_GUIDANCE: &str = "Hydrolysis renders through the Cherenkov GPU engine and has no CPU path: \
run on a machine with a GPU, or on a virtual machine enable hardware 3D acceleration and install the vendor driver. \
For targets without a GPU, waterui-dew is the CPU renderer. \
WGPU_BACKEND selects the wgpu backends the engine may pick an adapter from.";

/// The engine configuration Hydrolysis runs with: the backends `WGPU_BACKEND`
/// selects, GPU timestamps when profiling.
pub(crate) fn gpu_config(context: &str) -> cherenkov_gpu::GpuConfig {
    let backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::all());
    tracing::info!(target: "hydrolysis::gpu", context, ?backends, "creating cherenkov engine");
    cherenkov_gpu::GpuConfig {
        backends,
        timestamps: cfg!(feature = "frame-profile"),
        ..cherenkov_gpu::GpuConfig::default()
    }
}

fn log_selected_adapter(context: &str, engine: &Engine) {
    let info = engine.info();
    tracing::info!(
        target: "hydrolysis::gpu",
        context,
        adapter = %info.name,
        backend = %info.backend,
        device_type = %info.device_type,
        driver = %info.driver,
        driver_info = %info.driver_info,
        "selected wgpu adapter"
    );
}

/// Headless offscreen rendering surface: a readable Cherenkov target.
pub struct OffscreenSurface {
    gpu: OffscreenGpuContext,
    surface: cherenkov::Surface<cherenkov_gpu::Gpu>,
    width: u32,
    height: u32,
}

impl core::fmt::Debug for OffscreenSurface {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OffscreenSurface")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl OffscreenSurface {
    /// An offscreen surface on a fresh engine.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self::on_context(OffscreenGpuContext::new(), width, height)
    }

    /// An offscreen surface for WaterUI test hosts.
    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn new_for_tests(width: u32, height: u32) -> Self {
        Self::on_context(OffscreenGpuContext::new_for_tests(), width, height)
    }

    /// Creates a surface on an already-created [`OffscreenGpuContext`].
    ///
    /// # Panics
    /// Panics when the engine cannot create the target.
    #[must_use]
    pub fn on_context(gpu: OffscreenGpuContext, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let surface = gpu
            .engine
            .surface(cherenkov::Offscreen::new(
                (width, height),
                cherenkov::OffscreenFormat::LinearF16,
            ))
            .expect("hydrolysis offscreen surface: failed to create the offscreen target");
        Self {
            gpu,
            surface,
            width,
            height,
        }
    }

    /// The engine context the surface renders on.
    #[must_use]
    pub fn gpu(&self) -> &OffscreenGpuContext {
        &self.gpu
    }

    /// Reads the last rendered frame back.
    ///
    /// # Errors
    /// The engine's readback error.
    pub fn readback(&self) -> Result<cherenkov::Readback, cherenkov::RenderError> {
        self.surface.readback()
    }

    /// The last rendered frame as straight-alpha sRGB8 bytes, row-major.
    ///
    /// # Panics
    /// Panics when the surface cannot be read back.
    #[must_use]
    pub fn readback_rgba8(&self) -> Vec<u8> {
        crate::readback::readback_rgba8(&self.surface)
    }
}

impl SurfaceProvider for OffscreenSurface {
    fn engine(&self) -> &Rc<Engine> {
        &self.gpu.engine
    }

    fn surface(&self) -> &cherenkov::Surface<cherenkov_gpu::Gpu> {
        &self.surface
    }

    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if (width, height) != (self.width, self.height) {
            self.width = width;
            self.height = height;
            self.surface
                .resize((width, height))
                .expect("hydrolysis offscreen surface: the engine's render thread is gone");
        }
    }
}

/// Headless platform window backed by an offscreen texture.
#[derive(Debug)]
pub struct OffscreenWindow {
    surface: OffscreenSurface,
    scale_factor: f64,
    /// Last applied (min, max) content-size limits, recorded so tests can
    /// assert what the runner derived; offscreen surfaces have no real window
    /// to constrain.
    size_limits: Option<(
        Option<waterui_core::layout::Size>,
        Option<waterui_core::layout::Size>,
    )>,
}

impl OffscreenWindow {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            surface: OffscreenSurface::new(width, height),
            scale_factor: 1.0,
            size_limits: None,
        }
    }

    /// Creates an offscreen window for WaterUI test hosts.
    ///
    /// This keeps production adapter selection strict while allowing
    /// `waterui-testing` to run on compute-capable software adapters in CI.
    /// Requests a device of its own; a caller that builds several windows
    /// should request one [`OffscreenGpuContext`] and use [`Self::on_context`].
    #[cfg(any(test, feature = "testing"))]
    #[must_use]
    pub fn new_for_tests(width: u32, height: u32) -> Self {
        Self::on_context(OffscreenGpuContext::new_for_tests(), width, height)
    }

    /// Creates a window on an already-requested [`OffscreenGpuContext`], so
    /// every window built on that context shares its device.
    #[must_use]
    pub fn on_context(gpu: OffscreenGpuContext, width: u32, height: u32) -> Self {
        Self {
            surface: OffscreenSurface::on_context(gpu, width, height),
            scale_factor: 1.0,
            size_limits: None,
        }
    }

    /// Renders at `scale_factor` physical pixels per logical pixel.
    ///
    /// Layout stays in logical units; only the surface allocation and the
    /// reported [`PlatformWindow::scale_factor`] change, so a 2x offscreen
    /// window produces a HiDPI-sharp image of the very same layout.
    /// Sets the physical-pixels-per-logical-pixel ratio, reallocating the
    /// surface to match. See [`Self::with_scale_factor`].
    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        assert!(
            scale_factor.is_finite() && scale_factor > 0.0,
            "offscreen scale factor must be finite and positive, got {scale_factor}"
        );
        let logical_width = f64::from(self.surface.size().0) / self.scale_factor;
        let logical_height = f64::from(self.surface.size().1) / self.scale_factor;
        self.scale_factor = scale_factor;
        self.resize_to_logical(logical_width, logical_height);
    }

    #[must_use]
    pub fn with_scale_factor(mut self, scale_factor: f64) -> Self {
        assert!(
            scale_factor.is_finite() && scale_factor > 0.0,
            "offscreen scale factor must be finite and positive, got {scale_factor}"
        );
        self.set_scale_factor(scale_factor);
        self
    }

    fn resize_to_logical(&mut self, width: f64, height: f64) {
        let physical = |value: f64| (value * self.scale_factor).round().max(1.0) as u32;
        self.surface.resize(physical(width), physical(height));
    }

    #[must_use]
    pub fn surface_ref(&self) -> &OffscreenSurface {
        &self.surface
    }

    /// The last (min, max) content-size limits the runner applied, for tests.
    #[must_use]
    pub fn applied_size_limits(
        &self,
    ) -> Option<(
        Option<waterui_core::layout::Size>,
        Option<waterui_core::layout::Size>,
    )> {
        self.size_limits
    }
}

impl PlatformWindow for OffscreenWindow {
    fn surface(&mut self) -> &mut dyn SurfaceProvider {
        &mut self.surface
    }

    fn apply_properties(&mut self, window: &WuiWindow) {
        if window.state.snapshot() == WindowState::Closed {
            return;
        }
        let frame = validated_window_frame(window.frame.snapshot());
        // `frame` is in logical units; the surface is allocated in physical
        // pixels, so the scale factor has to be applied here or a HiDPI window
        // would rasterize at one physical pixel per logical pixel.
        self.resize_to_logical(
            f64::from(frame.width().max(1.0)),
            f64::from(frame.height().max(1.0)),
        );
    }

    fn set_size_limits(
        &mut self,
        min: Option<waterui_core::layout::Size>,
        max: Option<waterui_core::layout::Size>,
    ) {
        self.size_limits = Some((min, max));
    }

    fn applies_size_limits(&self) -> bool {
        true
    }

    fn drain_events(&mut self) -> Vec<InputEvent> {
        Vec::new()
    }

    fn request_redraw(&self) {}

    fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    fn sync_text_input_state(&mut self, _state: Option<TextInputState>) {}

    fn set_cursor_style(&mut self, _style: CursorStyle) {}
}

#[cfg(all(target_arch = "wasm32", feature = "web"))]
mod web_impl;

#[cfg(all(feature = "winit", target_os = "macos"))]
mod macos_display_link;

#[cfg(feature = "winit")]
mod winit_impl {
    use std::rc::Rc;
    use std::sync::Arc;

    use nami::Signal;
    use waterui::window::WindowState;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::{
        dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
        event::{
            ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase as WinitTouchPhase,
            WindowEvent,
        },
        keyboard::{Key, ModifiersState, PhysicalKey},
        window::{
            Cursor as WinitCursor, CursorIcon, Fullscreen, ImePurpose, Window as NativeWindow,
            WindowId,
        },
    };

    use super::{
        CursorStyle, Engine, InputEvent, KeyCode, KeyState, Modifiers, PlatformWindow,
        PointerButton, PointerKind, SurfaceProvider, TextInputPurpose, TextInputState,
        TouchPhase, validated_window_frame,
    };

    /// The engine every window of a winit runtime renders on: one device,
    /// one set of fonts and images, however many windows.
    #[derive(Clone)]
    pub struct WinitGpuContext {
        engine: Rc<Engine>,
    }

    impl WinitGpuContext {
        fn new() -> Self {
            let engine = Engine::new(super::gpu_config("hydrolysis winit surface")).unwrap_or_else(
                |error| {
                    panic!(
                        "hydrolysis winit surface: failed to create the Cherenkov engine: {error}. {}",
                        super::GPU_REQUIRED_GUIDANCE
                    )
                },
            );
            super::log_selected_adapter("hydrolysis winit surface", &engine);
            Self {
                engine: Rc::new(engine),
            }
        }

        /// The engine.
        #[must_use]
        pub fn engine(&self) -> &Rc<Engine> {
            &self.engine
        }
    }

    pub struct WinitSurface {
        surface: cherenkov::Surface<cherenkov_gpu::Gpu>,
        gpu: WinitGpuContext,
        size: (u32, u32),
    }

    impl core::fmt::Debug for WinitSurface {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("WinitSurface")
                .field("size", &self.size)
                .finish_non_exhaustive()
        }
    }

    impl WinitSurface {
        /// The X11 presentation defect a transparent window hits on an old
        /// Mesa software rasterizer: the WSI's `x11_present_to_x11_sw` sent
        /// its `xcb_put_image` at a hardcoded depth of 24, which the X
        /// server rejects with BadMatch for a depth-32 window — and the
        /// driver discards the reply, so `present` reports success while
        /// the window never updates. Fixed by Mesa commit 1e849b12
        /// ("vk/wsi/x11/sw: use swapchain depth for putimage"), released in
        /// Mesa 24.1. Returns the failure message naming the cause when
        /// `info` is that stack; `None` otherwise.
        ///
        /// The gate is the version alone: only a software rasterizer
        /// (`Cpu`, which is how Vulkan reports llvmpipe and lavapipe)
        /// presents through the software X11 WSI path at all, and it takes
        /// it on every X11 server it drives — with DRI3 it would take the
        /// shared-pixmap path instead, but a hardware Mesa driver already
        /// answers the device type differently, so a version check on the
        /// software adapter cannot misfire against hardware.
        fn mesa_x11_transparency_blocker(info: &cherenkov_gpu::GpuInfo) -> Option<String> {
            if info.device_type != "Cpu" {
                return None;
            }
            let (major, minor, patch) = Self::mesa_driver_version(&info.driver_info)?;
            ((major, minor) < (24, 1)).then(|| {
                format!(
                    "Mesa {major}.{minor}.{patch} software WSI presents \
                     depth-32 X11 windows at depth 24 and the server rejects \
                     every frame (fixed in Mesa 24.1, commit 1e849b12); \
                     upgrade Mesa"
                )
            })
        }

        /// The Mesa version `driver_info` announces, e.g. "Mesa
        /// 23.2.1-1ubuntu2". `None` for a non-Mesa driver or an
        /// unrecognizable string — an unversioned Mesa build is not proven
        /// broken, so it is not blocked.
        fn mesa_driver_version(driver_info: &str) -> Option<(u32, u32, u32)> {
            let version = &driver_info[driver_info.find("Mesa ")? + "Mesa ".len()..];
            let mut parts = version
                .split(|c: char| c != '.' && !c.is_ascii_digit())
                .next()?
                .split('.');
            let major = parts.next()?.parse().ok()?;
            let minor = parts.next().map_or(0, |p| p.parse().unwrap_or(0));
            let patch = parts.next().map_or(0, |p| p.parse().unwrap_or(0));
            Some((major, minor, patch))
        }

        /// Whether the realized winit window lives on an X11 connection —
        /// the only display path the Mesa software-WSI defect can hit. A
        /// window whose handle is not `Xcb`/`Xlib` (Wayland, AppKit,
        /// Windows, an unrecognized or missing handle) is not blocked.
        fn window_is_x11(window: &NativeWindow) -> bool {
            matches!(
                window.window_handle().map(|handle| handle.as_raw()),
                Ok(RawWindowHandle::Xcb(_) | RawWindowHandle::Xlib(_))
            )
        }

        /// A presentable surface on `window`, on `shared_gpu`'s engine when
        /// the runtime already has one.
        ///
        /// # Panics
        /// Panics when the engine cannot present to the window: no adapter,
        /// no transparency-capable composite mode for a transparent window,
        /// or the Mesa X11 software-WSI defect.
        pub fn new(
            window: Arc<NativeWindow>,
            shared_gpu: Option<&WinitGpuContext>,
            requires_transparency: bool,
        ) -> (Self, WinitGpuContext) {
            let gpu = shared_gpu.cloned().unwrap_or_else(WinitGpuContext::new);
            if requires_transparency
                && Self::window_is_x11(&window)
                && let Some(cause) = Self::mesa_x11_transparency_blocker(gpu.engine.info())
            {
                panic!("hydrolysis winit surface: {cause}");
            }
            let inner = window.inner_size();
            let size = (inner.width.max(1), inner.height.max(1));
            let target = cherenkov_gpu::WindowTarget::new(window, size)
                .transparent(requires_transparency);
            let surface = gpu.engine.surface(target).unwrap_or_else(|error| {
                panic!("hydrolysis winit surface: failed to create the window surface: {error}")
            });
            (
                Self {
                    surface,
                    gpu: gpu.clone(),
                    size,
                },
                gpu,
            )
        }
    }

    impl SurfaceProvider for WinitSurface {
        fn engine(&self) -> &Rc<Engine> {
            &self.gpu.engine
        }

        fn surface(&self) -> &cherenkov::Surface<cherenkov_gpu::Gpu> {
            &self.surface
        }

        fn size(&self) -> (u32, u32) {
            self.size
        }

        fn resize(&mut self, width: u32, height: u32) {
            let size = (width.max(1), height.max(1));
            if size != self.size {
                self.size = size;
                self.surface
                    .resize(size)
                    .expect("hydrolysis winit surface: the engine's render thread is gone");
            }
        }
    }

    /// The platform IME calls behind [`PlatformWindow::sync_text_input_state`],
    /// computed without a window: tracks the last state applied so a repeat
    /// sync is a no-op, `set_ime_allowed` fires only across a focus
    /// transition, and the cursor area is reported in physical pixels.
    #[derive(Debug, Default)]
    pub(crate) struct TextInputSync {
        applied: Option<TextInputState>,
    }

    /// One winit IME call [`TextInputSync::sync`] asks the window to make.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub(crate) enum TextInputSyncOp {
        Allowed(bool),
        Purpose(TextInputPurpose),
        CursorArea {
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        },
    }

    impl TextInputSync {
        /// The winit calls needed to bring the platform IME to `state`, in
        /// order. An unchanged state returns nothing — including when only
        /// the caret moved: a moved caret changes the state, so it is never
        /// swallowed by the equality early-return.
        pub(crate) fn sync(
            &mut self,
            state: Option<TextInputState>,
            scale_factor: f64,
        ) -> Vec<TextInputSyncOp> {
            if self.applied == state {
                return Vec::new();
            }
            let mut ops = Vec::with_capacity(3);
            if self.applied.is_some() != state.is_some() {
                ops.push(TextInputSyncOp::Allowed(state.is_some()));
            }
            self.applied = state;
            let Some(state) = state else {
                return ops;
            };
            ops.push(TextInputSyncOp::Purpose(state.purpose));
            assert!(
                scale_factor.is_finite() && scale_factor > 0.0,
                "hydrolysis winit backend received invalid scale factor {scale_factor}"
            );
            ops.push(TextInputSyncOp::CursorArea {
                x: (state.x * scale_factor).round() as i32,
                y: (state.y * scale_factor).round() as i32,
                width: (state.width.max(1.0) * scale_factor).ceil() as u32,
                height: (state.height.max(1.0) * scale_factor).ceil() as u32,
            });
            ops
        }
    }

    /// The window setup the app requested, held until the window has
    /// actually mapped.
    ///
    /// `apply_properties` pushes the requested frame and state while the
    /// window is still unmapped (`with_visible(false)`); on X11 a request
    /// made of an unmapped window is dropped and the window manager's own
    /// initial state wins the race — its placement for geometry, a normal
    /// window for `Fullscreen`/`Minimized`/`Closed`. The request is held
    /// for one re-delivery on the first event that only reaches a mapped
    /// window. Events that precede the map leave it armed.
    #[derive(Debug, Default)]
    struct MappedRequestRetry {
        pending: Option<PendingMappedRequest>,
        mapped: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct PendingMappedRequest {
        position: LogicalPosition<f64>,
        size: LogicalSize<f64>,
        state: WindowState,
    }

    impl MappedRequestRetry {
        /// Records the requested frame and state while the window is not
        /// yet known to be visible, overwriting any earlier pending one —
        /// the latest request wins. Once a mapped event has been seen this
        /// never re-arms: `is_visible` can lag the real map transition, and
        /// re-arming off it would re-deliver a request the live window has
        /// already overtaken.
        fn arm(
            &mut self,
            visible: Option<bool>,
            position: LogicalPosition<f64>,
            size: LogicalSize<f64>,
            state: WindowState,
        ) {
            if visible != Some(true) && !self.mapped {
                self.pending = Some(PendingMappedRequest {
                    position,
                    size,
                    state,
                });
            }
        }

        /// Consumes the held request on the first mapped-signal event —
        /// `Moved`, `Resized`, `Occluded(false)`, or `ScaleFactorChanged`,
        /// each of which only reaches a mapped window — returning it for
        /// re-application. Other events leave it armed.
        fn take_on_mapped_event(&mut self, event: &WindowEvent) -> Option<PendingMappedRequest> {
            let mapped = matches!(
                event,
                WindowEvent::Moved(_)
                    | WindowEvent::Resized(_)
                    | WindowEvent::ScaleFactorChanged { .. }
                    | WindowEvent::Occluded(false)
            );
            if mapped {
                self.mapped = true;
                self.pending.take()
            } else {
                None
            }
        }
    }

    /// Snapshot of the window properties `apply_properties` last pushed to the
    /// native window, so unchanged syncs cost no platform calls.
    #[derive(Clone, Debug, PartialEq)]
    struct AppliedWindowProperties {
        title: waterui::Str,
        resizable: bool,
        decorations: bool,
        state: WindowState,
        frame: waterui_core::layout::Rect,
    }

    #[derive(Debug)]
    pub struct WinitWindow {
        window: Arc<NativeWindow>,
        surface: WinitSurface,
        pending_surface_size: Option<PhysicalSize<u32>>,
        pending_events: Vec<InputEvent>,
        pointer_position: (f32, f32),
        modifiers: Modifiers,
        text_input_sync: TextInputSync,
        current_cursor_style: CursorStyle,
        /// Last applied (min, max) content-size limits, so per-frame application
        /// only reaches winit when the effective limits actually change.
        applied_size_limits: Option<(
            Option<waterui_core::layout::Size>,
            Option<waterui_core::layout::Size>,
        )>,
        /// Last applied window properties. `apply_properties` runs every event
        /// cycle, and each unconditional winit setter emits X writes whose
        /// replies wake the loop again — dedupe keeps the loop idle when
        /// nothing changed. The frame binding in particular is enforced only
        /// when it changed since the previous pump: a user-driven resize or
        /// move lands in the window server before its `Resized`/`Moved` event
        /// updates the binding, and reading the live geometry against the
        /// stale binding would yank the window straight back to its old frame.
        applied_properties: Option<AppliedWindowProperties>,
        /// The requested window frame and state held until the window has
        /// actually mapped (`with_visible(false)`): on X11 a request made
        /// of an unmapped window is dropped, so the first mapped-signal
        /// event re-delivers it — the window manager's own initial state
        /// otherwise wins.
        pending_mapped_request: MappedRequestRetry,
        /// Explicit ProMotion opt-in: declares the 120Hz frame-rate demand to
        /// the window server while redraws are being requested. `None` before
        /// macOS 14.
        #[cfg(target_os = "macos")]
        frame_rate_demand: Option<super::macos_display_link::FrameRateDemandLink>,
    }

    impl WinitWindow {
        pub fn new(window: Arc<NativeWindow>, requires_transparency: bool) -> Self {
            Self::new_with_shared_gpu(window, None, requires_transparency).0
        }

        pub fn new_with_shared_gpu(
            window: Arc<NativeWindow>,
            shared_gpu: Option<&WinitGpuContext>,
            requires_transparency: bool,
        ) -> (Self, WinitGpuContext) {
            let (surface, gpu) =
                WinitSurface::new(window.clone(), shared_gpu, requires_transparency);
            (
                Self {
                    #[cfg(target_os = "macos")]
                    frame_rate_demand: super::macos_display_link::FrameRateDemandLink::attach(
                        &window,
                    ),
                    window,
                    surface,
                    pending_surface_size: None,
                    pending_events: Vec::new(),
                    pointer_position: (0.0, 0.0),
                    modifiers: Modifiers::default(),
                    text_input_sync: TextInputSync::default(),
                    current_cursor_style: CursorStyle::Arrow,
                    applied_size_limits: None,
                    applied_properties: None,
                    pending_mapped_request: MappedRequestRetry::default(),
                },
                gpu,
            )
        }

        #[must_use]
        pub fn id(&self) -> WindowId {
            self.window.id()
        }

        #[must_use]
        pub fn native_window(&self) -> &NativeWindow {
            self.window.as_ref()
        }

        /// Pushes the requested `WindowState` to the window server. Shared
        /// by `apply_properties` and the first-mapped-event re-delivery: on
        /// X11 the same call made of an unmapped window is dropped.
        fn apply_window_state(&self, state: WindowState) {
            match state {
                WindowState::Normal => {
                    self.window.set_minimized(false);
                    self.window.set_fullscreen(None);
                }
                WindowState::Minimized => {
                    self.window.set_minimized(true);
                }
                WindowState::Fullscreen => {
                    self.window
                        .set_fullscreen(Some(Fullscreen::Borderless(None)));
                }
                WindowState::Closed => {
                    self.window.set_visible(false);
                }
            }
        }

        pub fn handle_window_event(&mut self, event: &WindowEvent) {
            // The first mapped-signal event re-applies the frame and state
            // the app asked for: requests made of an unmapped window were
            // dropped by X11, and the window manager's own state won the
            // race.
            if let Some(request) = self.pending_mapped_request.take_on_mapped_event(event) {
                self.window.set_outer_position(request.position);
                let _ = self.window.request_inner_size(request.size);
                self.apply_window_state(request.state);
            }
            match event {
                WindowEvent::CloseRequested => {
                    self.pending_events.push(InputEvent::CloseRequested);
                }
                WindowEvent::Resized(size) => {
                    self.pending_surface_size = Some(*size);
                    self.pending_events.push(InputEvent::Resize {
                        width: size.width.max(1),
                        height: size.height.max(1),
                    });
                }
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    assert!(
                        scale_factor.is_finite() && *scale_factor > 0.0,
                        "hydrolysis winit backend received invalid scale factor {scale_factor}"
                    );
                    let size = self.window.inner_size();
                    self.pending_surface_size = Some(size);
                    self.pending_events.push(InputEvent::Resize {
                        width: size.width.max(1),
                        height: size.height.max(1),
                    });
                }
                WindowEvent::Moved(position) => {
                    let logical = position.to_logical::<f64>(self.window.scale_factor());
                    self.pending_events.push(InputEvent::Moved {
                        x: logical.x as f32,
                        y: logical.y as f32,
                    });
                }
                WindowEvent::Focused(focused) => {
                    self.pending_events.push(InputEvent::Focused(*focused));
                }
                WindowEvent::CursorMoved { position, .. } => {
                    self.pointer_position =
                        map_cursor_position(position, self.window.scale_factor());
                    tracing::trace!(
                        target: "waterui::hydrolysis::input_raw",
                        event = "cursor_moved",
                        x = self.pointer_position.0,
                        y = self.pointer_position.1,
                        "winit raw input event"
                    );
                    self.pending_events.push(InputEvent::PointerMove {
                        id: 0,
                        kind: PointerKind::Mouse,
                        x: self.pointer_position.0,
                        y: self.pointer_position.1,
                    });
                }
                WindowEvent::CursorLeft { .. } => {
                    self.pending_events.push(InputEvent::PointerCancel {
                        id: 0,
                        kind: PointerKind::Mouse,
                    });
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    let mapped_button = map_button(*button);
                    let (x, y) = self.pointer_position;
                    tracing::trace!(
                        target: "waterui::hydrolysis::input_raw",
                        event = "mouse_input",
                        x,
                        y,
                        state = ?state,
                        button = ?mapped_button,
                        "winit raw input event"
                    );
                    match state {
                        ElementState::Pressed => {
                            self.pending_events.push(InputEvent::PointerDown {
                                id: 0,
                                kind: PointerKind::Mouse,
                                x,
                                y,
                                button: mapped_button,
                            });
                        }
                        ElementState::Released => {
                            self.pending_events.push(InputEvent::PointerUp {
                                id: 0,
                                kind: PointerKind::Mouse,
                                x,
                                y,
                                button: mapped_button,
                            });
                        }
                    }
                }
                WindowEvent::Touch(touch) => {
                    let position = map_cursor_position(&touch.location, self.window.scale_factor());
                    self.pointer_position = position;
                    let (x, y) = position;
                    let event = match touch.phase {
                        WinitTouchPhase::Started => InputEvent::PointerDown {
                            id: touch.id,
                            kind: PointerKind::Touch,
                            x,
                            y,
                            button: PointerButton::Primary,
                        },
                        WinitTouchPhase::Moved => InputEvent::PointerMove {
                            id: touch.id,
                            kind: PointerKind::Touch,
                            x,
                            y,
                        },
                        WinitTouchPhase::Ended => InputEvent::PointerUp {
                            id: touch.id,
                            kind: PointerKind::Touch,
                            x,
                            y,
                            button: PointerButton::Primary,
                        },
                        WinitTouchPhase::Cancelled => InputEvent::PointerCancel {
                            id: touch.id,
                            kind: PointerKind::Touch,
                        },
                    };
                    self.pending_events.push(event);
                }
                WindowEvent::MouseWheel { delta, phase, .. } => {
                    let (dx, dy, is_line_delta) =
                        map_scroll_delta(delta, self.window.scale_factor());
                    if is_line_delta {
                        self.pending_events.push(InputEvent::Scroll {
                            x: self.pointer_position.0,
                            y: self.pointer_position.1,
                            dx,
                            dy,
                            is_line_delta,
                        });
                    } else {
                        self.pending_events.push(InputEvent::TrackpadPan {
                            x: self.pointer_position.0,
                            y: self.pointer_position.1,
                            dx,
                            dy,
                            phase: map_touch_phase(*phase),
                        });
                    }
                }
                WindowEvent::PinchGesture { delta, phase, .. } => {
                    self.pending_events.push(InputEvent::Magnification {
                        x: self.pointer_position.0,
                        y: self.pointer_position.1,
                        delta: *delta as f32,
                        phase: map_touch_phase(*phase),
                    });
                }
                WindowEvent::RotationGesture { delta, phase, .. } => {
                    self.pending_events.push(InputEvent::Rotation {
                        x: self.pointer_position.0,
                        y: self.pointer_position.1,
                        delta: *delta,
                        phase: map_touch_phase(*phase),
                    });
                }
                WindowEvent::ModifiersChanged(modifiers) => {
                    self.modifiers = modifiers.state().into();
                    self.pending_events
                        .push(InputEvent::ModifiersChanged(self.modifiers));
                }
                WindowEvent::KeyboardInput {
                    event,
                    is_synthetic,
                    ..
                } => queue_keyboard_input(
                    &mut self.pending_events,
                    self.modifiers,
                    WinitKeyInput {
                        is_synthetic: *is_synthetic,
                        state: event.state,
                        repeat: event.repeat,
                        text: event.text.as_deref(),
                        logical_key: &event.logical_key,
                        physical_key: event.physical_key,
                    },
                ),
                WindowEvent::Ime(ime) => match ime {
                    Ime::Preedit(text, caret) => {
                        tracing::trace!(
                            target: "waterui::hydrolysis::input_raw",
                            event = "ime_preedit",
                            text = text.as_str(),
                            "winit raw input event"
                        );
                        self.pending_events.push(InputEvent::ImePreedit {
                            text: text.clone(),
                            // winit reports the pre-edit selection as a byte
                            // range; the caret sits at its start.
                            caret: caret.map(|(start, _)| start),
                        });
                    }
                    Ime::Commit(text) => {
                        tracing::trace!(
                            target: "waterui::hydrolysis::input_raw",
                            event = "ime_commit",
                            text = text.as_str(),
                            "winit raw input event"
                        );
                        self.pending_events
                            .push(InputEvent::ImeCommit { text: text.clone() });
                    }
                    Ime::Disabled => {
                        tracing::trace!(
                            target: "waterui::hydrolysis::input_raw",
                            event = "ime_disabled",
                            "winit raw input event"
                        );
                        self.pending_events.push(InputEvent::ImeDisabled);
                    }
                    Ime::Enabled => {}
                },
                _ => {}
            }
        }
    }

    fn map_cursor_position(position: &PhysicalPosition<f64>, scale_factor: f64) -> (f32, f32) {
        assert!(
            scale_factor.is_finite() && scale_factor > 0.0,
            "hydrolysis winit backend received invalid scale factor {scale_factor}"
        );
        let logical = position.to_logical::<f64>(scale_factor);
        (logical.x as f32, logical.y as f32)
    }

    fn map_scroll_delta(delta: &MouseScrollDelta, scale_factor: f64) -> (f32, f32, bool) {
        assert!(
            scale_factor.is_finite() && scale_factor > 0.0,
            "hydrolysis winit backend received invalid scale factor {scale_factor}"
        );
        match delta {
            MouseScrollDelta::LineDelta(dx, dy) => (*dx, *dy, true),
            MouseScrollDelta::PixelDelta(delta) => {
                let logical = delta.to_logical::<f64>(scale_factor);
                (logical.x as f32, logical.y as f32, false)
            }
        }
    }

    impl PlatformWindow for WinitWindow {
        fn surface(&mut self) -> &mut dyn SurfaceProvider {
            if let Some(size) = self.pending_surface_size.take() {
                self.surface.resize(size.width, size.height);
            }
            &mut self.surface
        }

        fn applies_size_limits(&self) -> bool {
            true
        }

        fn set_size_limits(
            &mut self,
            min: Option<waterui_core::layout::Size>,
            max: Option<waterui_core::layout::Size>,
        ) {
            if self.applied_size_limits == Some((min, max)) {
                return;
            }
            self.window.set_min_inner_size(
                min.map(|size| LogicalSize::new(f64::from(size.width), f64::from(size.height))),
            );
            self.window.set_max_inner_size(
                max.map(|size| LogicalSize::new(f64::from(size.width), f64::from(size.height))),
            );
            self.applied_size_limits = Some((min, max));
        }

        fn apply_properties(&mut self, window: &waterui::window::Window) {
            let title = window.display_title().snapshot();
            let decorations = !matches!(window.style, waterui::window::WindowStyle::Borderless);
            let state = window.state.snapshot();
            let frame = validated_window_frame(window.frame.snapshot());
            let properties = AppliedWindowProperties {
                title: title.clone(),
                resizable: window.resizable,
                decorations,
                state,
                frame,
            };
            let previous = self.applied_properties.replace(properties.clone());
            let applied = previous.as_ref();
            if applied.is_none_or(|p| p.title != properties.title) {
                self.window.set_title(properties.title.as_str());
            }
            if applied.is_none_or(|p| p.resizable != properties.resizable) {
                self.window.set_resizable(properties.resizable);
            }
            if applied.is_none_or(|p| p.decorations != properties.decorations) {
                self.window.set_decorations(properties.decorations);
            }
            // The frame binding is pushed to the window only when it changed
            // since the previous pump. A user-driven resize or move lands in
            // the window server before its `Resized`/`Moved` event reaches the
            // binding, so the live geometry legitimately disagrees with the
            // stale binding in that window — enforcing it then would yank the
            // window straight back and make user resizes impossible.
            // Arm the post-map re-apply while the window is still hidden:
            // the frame and state pushed above reach an unmapped X11 window
            // and are dropped, so the first mapped event must re-deliver
            // them — a window manager that stretched the window on map is
            // put back on the requested frame, and a Fullscreen/Minimized/
            // Closed request is restored after the window exists. Once the
            // window is visible the app-requested setup was already
            // enforced and further changes just flow through the ordinary
            // path.
            let target_size = LogicalSize::new(frame.width() as f64, frame.height() as f64);
            let mut target_position = LogicalPosition::new(frame.x() as f64, frame.y() as f64);
            if let Some(monitor) = self.window.current_monitor() {
                let scale_factor = self.window.scale_factor();
                let monitor_position = monitor.position().to_logical::<f64>(scale_factor);
                let monitor_size = monitor.size().to_logical::<f64>(scale_factor);
                let max_x = (monitor_position.x + monitor_size.width - target_size.width)
                    .max(monitor_position.x);
                let max_y = (monitor_position.y + monitor_size.height - target_size.height)
                    .max(monitor_position.y);
                target_position.x = target_position.x.clamp(monitor_position.x, max_x);
                target_position.y = target_position.y.clamp(monitor_position.y, max_y);
            }
            self.pending_mapped_request.arm(
                self.window.is_visible(),
                target_position,
                target_size,
                state,
            );
            let size_changed = applied.is_none_or(|p| *p.frame.size() != *frame.size());
            let origin_changed = applied.is_none_or(|p| p.frame.origin() != frame.origin());
            if size_changed || origin_changed {
                let current_position = self
                    .window
                    .outer_position()
                    .ok()
                    .map(|value| value.to_logical::<f64>(self.window.scale_factor()));
                if origin_changed
                    && current_position.is_none_or(|current| {
                        (current.x - target_position.x).abs() > 0.5
                            || (current.y - target_position.y).abs() > 0.5
                    })
                {
                    self.window.set_outer_position(target_position);
                }
                let current_size = self
                    .window
                    .inner_size()
                    .to_logical::<f64>(self.window.scale_factor());
                if size_changed
                    && ((current_size.width - target_size.width).abs() > 0.5
                        || (current_size.height - target_size.height).abs() > 0.5)
                {
                    let _ = self.window.request_inner_size(target_size);
                }
            }
            if applied.is_none_or(|p| p.state != state) {
                self.apply_window_state(state);
            }
        }

        fn drain_events(&mut self) -> Vec<InputEvent> {
            core::mem::take(&mut self.pending_events)
        }

        fn request_redraw(&self) {
            self.window.request_redraw();
            // Hold the ProMotion frame-rate demand while frames are being
            // requested, so animations run at 120Hz on high-refresh panels.
            #[cfg(target_os = "macos")]
            if let Some(demand) = &self.frame_rate_demand {
                demand.hold_demand();
            }
        }


        fn scale_factor(&self) -> f64 {
            self.window.scale_factor()
        }

        fn refresh_rate_hz(&self) -> Option<f64> {
            self.window
                .current_monitor()
                .and_then(|monitor| monitor.refresh_rate_millihertz())
                .map(|millihertz| f64::from(millihertz) / 1000.0)
        }

        fn sync_text_input_state(&mut self, state: Option<TextInputState>) {
            let scale_factor = self.window.scale_factor();
            for op in self.text_input_sync.sync(state, scale_factor) {
                match op {
                    TextInputSyncOp::Allowed(allowed) => {
                        self.window.set_ime_allowed(allowed);
                    }
                    TextInputSyncOp::Purpose(purpose) => {
                        let purpose = match purpose {
                            TextInputPurpose::Normal => ImePurpose::Normal,
                            TextInputPurpose::Password => ImePurpose::Password,
                        };
                        self.window.set_ime_purpose(purpose);
                    }
                    TextInputSyncOp::CursorArea {
                        x,
                        y,
                        width,
                        height,
                    } => {
                        self.window.set_ime_cursor_area(
                            PhysicalPosition::new(x, y),
                            PhysicalSize::new(width, height),
                        );
                    }
                }
            }
        }

        fn set_cursor_style(&mut self, style: CursorStyle) {
            if self.current_cursor_style == style {
                return;
            }
            self.current_cursor_style = style;
            self.window
                .set_cursor(WinitCursor::Icon(map_cursor_style(style)));
        }
    }

    impl From<ModifiersState> for Modifiers {
        fn from(value: ModifiersState) -> Self {
            Self {
                shift: value.shift_key(),
                control: value.control_key(),
                alt: value.alt_key(),
                super_key: value.super_key(),
            }
        }
    }

    fn map_touch_phase(phase: WinitTouchPhase) -> TouchPhase {
        match phase {
            WinitTouchPhase::Started => TouchPhase::Started,
            WinitTouchPhase::Moved => TouchPhase::Moved,
            WinitTouchPhase::Ended => TouchPhase::Ended,
            WinitTouchPhase::Cancelled => TouchPhase::Cancelled,
        }
    }

    fn map_button(button: MouseButton) -> PointerButton {
        match button {
            MouseButton::Left => PointerButton::Primary,
            MouseButton::Right => PointerButton::Secondary,
            MouseButton::Middle => PointerButton::Middle,
            MouseButton::Back => PointerButton::Back,
            MouseButton::Forward => PointerButton::Forward,
            MouseButton::Other(value) => PointerButton::Other(value),
        }
    }

    fn map_key(key: &Key) -> KeyCode {
        match key {
            Key::Character(value) => KeyCode::Character(value.to_string()),
            Key::Named(value) => KeyCode::Named(format!("{value:?}")),
            _ => KeyCode::Unidentified,
        }
    }

    fn should_emit_keyboard_text(modifiers: Modifiers) -> bool {
        !(modifiers.control || modifiers.alt || modifiers.super_key)
    }

    /// The fields of `WindowEvent::KeyboardInput` hydrolysis reads, decomposed
    /// at the match site: `winit::event::KeyEvent` cannot be constructed
    /// outside winit (its `platform_specific` field is private), and this
    /// translation is what the unit tests drive.
    #[derive(Clone, Copy)]
    struct WinitKeyInput<'a> {
        /// winit's focus-change replay (on X11, `XI_FocusIn` resends every
        /// held key as a synthetic press and `XI_FocusOut` as a synthetic
        /// release): state synchronisation, not a keystroke the user made.
        is_synthetic: bool,
        state: ElementState,
        repeat: bool,
        text: Option<&'a str>,
        logical_key: &'a Key,
        physical_key: PhysicalKey,
    }

    fn queue_keyboard_input(
        pending_events: &mut Vec<InputEvent>,
        modifiers: Modifiers,
        input: WinitKeyInput<'_>,
    ) {
        if input.is_synthetic {
            // A replayed press carries no user input: no text, key action or
            // gesture may observe it. The modifier side of the same sync
            // still arrives through `ModifiersChanged`.
            if input.state == ElementState::Released {
                // The focus-out replay of a held key's release aborts the
                // press it belonged to: nothing downstream may activate on
                // it, but the armed press state must come down so a real
                // release later cannot fire a stale target.
                pending_events.push(InputEvent::KeyboardCancel);
            }
            return;
        }
        if input.state == ElementState::Pressed
            && should_emit_keyboard_text(modifiers)
            && let Some(text) = keyboard_text_payload(input.text)
        {
            tracing::trace!(
                target: "waterui::hydrolysis::input_raw",
                event = "keyboard_text",
                text,
                "winit raw input event"
            );
            pending_events.push(InputEvent::TextInput {
                text: text.to_string(),
            });
        }
        tracing::trace!(
            target: "waterui::hydrolysis::input_raw",
            event = "keyboard_input",
            state = ?input.state,
            logical_key = ?input.logical_key,
            modifiers = ?modifiers,
            "winit raw input event"
        );
        pending_events.push(InputEvent::Key {
            key: map_key_event(input.logical_key, input.text, modifiers),
            logical_key: ui_events_winit::keyboard::from_winit_key(input.logical_key.clone()),
            physical_code: ui_events_winit::keyboard::from_winit_code(input.physical_key),
            repeat: input.repeat,
            state: match input.state {
                ElementState::Pressed => KeyState::Pressed,
                ElementState::Released => KeyState::Released,
            },
            modifiers,
        });
    }

    fn map_key_event(logical_key: &Key, text: Option<&str>, modifiers: Modifiers) -> KeyCode {
        if should_emit_keyboard_text(modifiers)
            && keyboard_text_payload(text).is_some()
            && matches!(logical_key, Key::Character(_))
        {
            return KeyCode::Unidentified;
        }
        map_key(logical_key)
    }

    fn keyboard_text_payload(text: Option<&str>) -> Option<&str> {
        let text = text?;
        if text.is_empty() || text.chars().all(char::is_control) {
            return None;
        }
        Some(text)
    }

    fn map_cursor_style(style: CursorStyle) -> CursorIcon {
        match style {
            CursorStyle::Arrow => CursorIcon::Default,
            CursorStyle::PointingHand => CursorIcon::Pointer,
            CursorStyle::IBeam => CursorIcon::Text,
            CursorStyle::Crosshair => CursorIcon::Crosshair,
            CursorStyle::OpenHand => CursorIcon::Grab,
            CursorStyle::ClosedHand => CursorIcon::Grabbing,
            CursorStyle::NotAllowed => CursorIcon::NotAllowed,
            CursorStyle::ResizeLeft => CursorIcon::WResize,
            CursorStyle::ResizeRight => CursorIcon::EResize,
            CursorStyle::ResizeUp => CursorIcon::NResize,
            CursorStyle::ResizeDown => CursorIcon::SResize,
            CursorStyle::ResizeLeftRight => CursorIcon::EwResize,
            CursorStyle::ResizeUpDown => CursorIcon::NsResize,
            CursorStyle::Move => CursorIcon::Move,
            CursorStyle::Wait => CursorIcon::Wait,
            CursorStyle::Copy => CursorIcon::Copy,
            _ => panic!("unsupported CursorStyle variant in hydrolysis winit backend"),
        }
    }

    pub use WinitGpuContext as ExportedWinitGpuContext;
    pub use WinitWindow as ExportedWinitWindow;

    #[cfg(test)]
    mod tests {
        use winit::dpi::PhysicalPosition;
        use winit::event::{ElementState, MouseScrollDelta};
        use winit::keyboard::{Key, PhysicalKey};

        use super::{
            TextInputSync, TextInputSyncOp, WinitKeyInput, map_cursor_position, map_scroll_delta,
            queue_keyboard_input, should_emit_keyboard_text,
        };
        use crate::platform::{InputEvent, KeyState, Modifiers, TextInputPurpose, TextInputState};

        fn input_state(x: f64, y: f64, purpose: TextInputPurpose) -> TextInputState {
            TextInputState {
                x,
                y,
                width: 2.0,
                height: 14.0,
                purpose,
            }
        }

        #[test]
        fn sync_reports_allowed_only_across_focus_transitions() {
            let mut sync = TextInputSync::default();
            let state = input_state(10.0, 20.0, TextInputPurpose::Normal);
            assert_eq!(
                sync.sync(Some(state), 1.0),
                vec![
                    TextInputSyncOp::Allowed(true),
                    TextInputSyncOp::Purpose(TextInputPurpose::Normal),
                    TextInputSyncOp::CursorArea {
                        x: 10,
                        y: 20,
                        width: 2,
                        height: 14,
                    },
                ]
            );
            // Same state again: nothing to do.
            assert_eq!(sync.sync(Some(state), 1.0), Vec::new());
            // Losing focus disables the IME once and reports nothing else.
            assert_eq!(sync.sync(None, 1.0), vec![TextInputSyncOp::Allowed(false)]);
            assert_eq!(sync.sync(None, 1.0), Vec::new());
            // Refocusing re-enables it.
            assert_eq!(
                sync.sync(Some(state), 1.0),
                vec![
                    TextInputSyncOp::Allowed(true),
                    TextInputSyncOp::Purpose(TextInputPurpose::Normal),
                    TextInputSyncOp::CursorArea {
                        x: 10,
                        y: 20,
                        width: 2,
                        height: 14,
                    },
                ]
            );
        }

        #[test]
        fn sync_reports_purpose_transitions() {
            let mut sync = TextInputSync::default();
            let normal = input_state(10.0, 20.0, TextInputPurpose::Normal);
            let password = input_state(10.0, 20.0, TextInputPurpose::Password);
            let _ = sync.sync(Some(normal), 1.0);
            // Purpose changed while staying allowed: no Allowed op, but the
            // new purpose and cursor area are reported.
            assert_eq!(
                sync.sync(Some(password), 1.0),
                vec![
                    TextInputSyncOp::Purpose(TextInputPurpose::Password),
                    TextInputSyncOp::CursorArea {
                        x: 10,
                        y: 20,
                        width: 2,
                        height: 14,
                    },
                ]
            );
        }

        #[test]
        fn sync_converts_logical_geometry_to_physical_pixels() {
            let mut sync = TextInputSync::default();
            let ops = sync.sync(Some(input_state(10.4, 20.5, TextInputPurpose::Normal)), 2.0);
            assert_eq!(
                ops.last(),
                Some(&TextInputSyncOp::CursorArea {
                    x: 21,      // 10.4 * 2.0 rounded
                    y: 41,      // 20.5 * 2.0 rounded
                    width: 4,   // max(2,1) * 2.0 ceiled
                    height: 28, // max(14,1) * 2.0 ceiled
                })
            );
        }

        #[test]
        fn sync_never_swallows_a_moving_caret() {
            let mut sync = TextInputSync::default();
            let _ = sync.sync(Some(input_state(10.0, 20.0, TextInputPurpose::Normal)), 1.0);
            // Only the caret x moves: the state differs, so the equality
            // early-return must not drop the update.
            let ops = sync.sync(Some(input_state(12.0, 20.0, TextInputPurpose::Normal)), 1.0);
            assert_eq!(
                ops,
                vec![
                    TextInputSyncOp::Purpose(TextInputPurpose::Normal),
                    TextInputSyncOp::CursorArea {
                        x: 12,
                        y: 20,
                        width: 2,
                        height: 14,
                    },
                ]
            );
        }

        #[test]
        fn cursor_position_is_converted_to_logical_coordinates() {
            let (x, y) = map_cursor_position(&PhysicalPosition::new(384.5, 216.25), 2.0);
            assert_eq!(x, 192.25);
            assert_eq!(y, 108.125);
        }

        #[test]
        fn pixel_scroll_delta_is_converted_to_logical_space() {
            let (dx, dy, is_line_delta) = map_scroll_delta(
                &MouseScrollDelta::PixelDelta(PhysicalPosition::new(120.0, -48.5)),
                2.0,
            );
            assert_eq!(dx, 60.0);
            assert_eq!(dy, -24.25);
            assert!(!is_line_delta);
        }

        #[test]
        fn line_scroll_delta_is_preserved() {
            let (dx, dy, is_line_delta) =
                map_scroll_delta(&MouseScrollDelta::LineDelta(-2.0, 3.5), 2.0);
            assert_eq!(dx, -2.0);
            assert_eq!(dy, 3.5);
            assert!(is_line_delta);
        }

        #[test]
        fn command_modified_characters_are_reserved_for_shortcuts() {
            assert!(should_emit_keyboard_text(Modifiers {
                shift: true,
                ..Modifiers::default()
            }));
            assert!(!should_emit_keyboard_text(Modifiers {
                control: true,
                ..Modifiers::default()
            }));
            assert!(!should_emit_keyboard_text(Modifiers {
                super_key: true,
                ..Modifiers::default()
            }));
            assert!(!should_emit_keyboard_text(Modifiers {
                alt: true,
                ..Modifiers::default()
            }));
        }

        /// water-rs/hydrolysis#211: on X11, `XI_FocusIn` replays every held
        /// key as a synthetic `KeyboardInput` press and `XI_FocusOut` as a
        /// synthetic release — state synchronisation, not keystrokes. The
        /// replayed press must not produce text or a key event; the replayed
        /// release surfaces only as the cancellation of the press it paired
        /// with. The real press that follows is the only one that types.
        #[test]
        fn a_synthetic_focus_replay_emits_no_input() {
            let logical_e = Key::Character("e".into());
            let held_key = WinitKeyInput {
                is_synthetic: true,
                state: ElementState::Pressed,
                repeat: false,
                text: Some("e"),
                logical_key: &logical_e,
                physical_key: PhysicalKey::Code(winit::keyboard::KeyCode::KeyE),
            };

            let mut events = Vec::new();
            // The focus-in replay of the held key, then the real press.
            queue_keyboard_input(&mut events, Modifiers::default(), held_key);
            queue_keyboard_input(
                &mut events,
                Modifiers::default(),
                WinitKeyInput {
                    state: ElementState::Released,
                    ..held_key
                },
            );
            queue_keyboard_input(
                &mut events,
                Modifiers::default(),
                WinitKeyInput {
                    is_synthetic: false,
                    ..held_key
                },
            );

            let text_inputs = events
                .iter()
                .filter(|event| matches!(event, InputEvent::TextInput { .. }))
                .count();
            let presses = events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        InputEvent::Key {
                            state: KeyState::Pressed,
                            ..
                        }
                    )
                })
                .count();
            let cancels = events
                .iter()
                .filter(|event| matches!(event, InputEvent::KeyboardCancel))
                .count();
            assert_eq!(text_inputs, 1, "only the real press may type text");
            assert_eq!(presses, 1, "only the real press may produce a key event");
            assert_eq!(
                cancels, 1,
                "the synthetic release cancels the press it belonged to"
            );
            assert_eq!(events.len(), 3, "the synthetic press emits nothing");
        }

        #[test]
        fn cursor_position_panics_with_invalid_scale_factor() {
            let result = std::panic::catch_unwind(|| {
                let _ = map_cursor_position(&PhysicalPosition::new(120.0, 80.0), 0.0);
            });
            assert!(result.is_err());
        }

        fn caps_with_alpha_modes(
            alpha_modes: &[wgpu::CompositeAlphaMode],
        ) -> wgpu::SurfaceCapabilities {
            wgpu::SurfaceCapabilities {
                alpha_modes: alpha_modes.to_vec(),
                ..wgpu::SurfaceCapabilities::default()
            }
        }

        fn fake_adapter_info() -> wgpu::AdapterInfo {
            wgpu::AdapterInfo {
                name: "fake adapter".to_string(),
                vendor: 0,
                device: 0,
                device_type: wgpu::DeviceType::Cpu,
                device_pci_bus_id: String::new(),
                driver: String::new(),
                driver_info: String::new(),
                backend: wgpu::Backend::Gl,
                subgroup_min_size: 4,
                subgroup_max_size: 128,
                transient_saves_memory: false,
            }
        }

        #[test]
        fn a_transparent_window_gets_the_first_transparent_alpha_mode_the_surface_offers() {
            use wgpu::CompositeAlphaMode as Mode;
            assert_eq!(
                super::WinitSurface::select_alpha_mode(
                    &caps_with_alpha_modes(&[Mode::Opaque, Mode::Inherit, Mode::PreMultiplied]),
                    true,
                    &fake_adapter_info(),
                ),
                Mode::PreMultiplied
            );
            // X11 compositing on a 32-bit visual reports exactly this pair:
            // with no explicit multiplied mode the inherited mode is the only
            // one whose alpha reaches the compositor.
            assert_eq!(
                super::WinitSurface::select_alpha_mode(
                    &caps_with_alpha_modes(&[Mode::Opaque, Mode::Inherit]),
                    true,
                    &fake_adapter_info(),
                ),
                Mode::Inherit
            );
        }

        #[test]
        fn an_opaque_window_keeps_the_surface_preferred_alpha_mode() {
            use wgpu::CompositeAlphaMode as Mode;
            assert_eq!(
                super::WinitSurface::select_alpha_mode(
                    &caps_with_alpha_modes(&[Mode::Opaque, Mode::PreMultiplied]),
                    false,
                    &fake_adapter_info(),
                ),
                Mode::Opaque
            );
        }

        /// water-rs/hydrolysis#118: a depth-32 X11 window on a Mesa
        /// software rasterizer below 24.1 is silently un-presentable — the
        /// version gate is the only signal presentation never had.
        #[test]
        fn a_mesa_software_adapter_below_24_1_is_blocked_for_transparency() {
            use super::WinitSurface;
            let adapter = |driver_info: &str, device_type: wgpu::DeviceType| {
                let mut info = fake_adapter_info();
                info.name = "llvmpipe (LLVM 15.0.7, 256 bits)".to_string();
                info.device_type = device_type;
                info.driver_info = driver_info.to_string();
                info
            };
            let blocked = WinitSurface::mesa_x11_transparency_blocker(&adapter(
                "Mesa 23.2.1-1ubuntu2",
                wgpu::DeviceType::Cpu,
            ));
            let cause = blocked.expect("Mesa 23.2.1 llvmpipe must be rejected");
            assert!(
                cause.contains("23.2.1")
                    && cause.contains("Mesa 24.1")
                    && cause.contains("1e849b12")
                    && cause.contains("upgrade Mesa"),
                "the failure must name the cause and the fix: {cause}"
            );
            for driver_info in [
                "Mesa 24.1.0",
                "Mesa 24.1.0-devel (git-c4b20fd)",
                "Mesa 25.0.7",
                "Mesa 26.2.3",
                // Not a Mesa driver — the defect is Mesa-specific.
                "SwiftShader driver",
                // No Mesa version to prove the defect against.
                "",
            ] {
                assert!(
                    WinitSurface::mesa_x11_transparency_blocker(&adapter(
                        driver_info,
                        wgpu::DeviceType::Cpu,
                    ))
                    .is_none(),
                    "{driver_info} must pass"
                );
            }
            // A hardware adapter presents through DRI3, never the software
            // X11 WSI path — even a Mesa one on an old version.
            for device_type in [
                wgpu::DeviceType::IntegratedGpu,
                wgpu::DeviceType::DiscreteGpu,
                wgpu::DeviceType::VirtualGpu,
                wgpu::DeviceType::Other,
            ] {
                assert!(
                    WinitSurface::mesa_x11_transparency_blocker(&adapter(
                        "Mesa 23.2.1",
                        device_type,
                    ))
                    .is_none(),
                    "{device_type:?} must pass"
                );
            }
        }

        #[test]
        #[should_panic(expected = "fake adapter")]
        fn a_transparent_window_on_an_opaque_only_adapter_fails_at_creation() {
            use wgpu::CompositeAlphaMode as Mode;
            let _ = super::WinitSurface::select_alpha_mode(
                &caps_with_alpha_modes(&[Mode::Opaque]),
                true,
                &fake_adapter_info(),
            );
        }

        #[test]
        fn the_requested_frame_is_reapplied_on_the_first_mapped_event() {
            use waterui::window::WindowState;
            use winit::dpi::{LogicalPosition, LogicalSize, PhysicalSize};
            use winit::event::WindowEvent;

            let mut retry = super::MappedRequestRetry::default();
            let position = LogicalPosition::new(12.0, 34.0);
            let size = LogicalSize::new(800.0, 300.0);
            let armed = |position, size, state| super::PendingMappedRequest {
                position,
                size,
                state,
            };

            // Armed while the window is unmapped: a pre-map event leaves the
            // frame held; the first mapped-signal event returns it once.
            retry.arm(Some(false), position, size, WindowState::Normal);
            assert_eq!(
                retry.take_on_mapped_event(&WindowEvent::Focused(true)),
                None,
                "a non-mapped event must not consume the held frame"
            );
            assert_eq!(
                retry.take_on_mapped_event(&WindowEvent::Moved(PhysicalPosition::new(0, 0))),
                Some(armed(position, size, WindowState::Normal)),
            );
            assert_eq!(
                retry.take_on_mapped_event(&WindowEvent::Resized(PhysicalSize::new(1, 1))),
                None,
                "the re-apply fires once only"
            );

            // A later request while still hidden replaces the earlier one —
            // frame and state alike.
            let mut retry = super::MappedRequestRetry::default();
            retry.arm(Some(false), position, size, WindowState::Normal);
            let newer = (
                LogicalPosition::new(56.0, 78.0),
                LogicalSize::new(1024.0, 640.0),
            );
            retry.arm(None, newer.0, newer.1, WindowState::Fullscreen);
            assert_eq!(
                retry.take_on_mapped_event(&WindowEvent::Occluded(false)),
                Some(armed(newer.0, newer.1, WindowState::Fullscreen)),
            );

            // Once the window is visible nothing is held: the ordinary
            // frame-binding path applies later frames. `is_visible` may lag
            // the real map transition — a mapped event latches `mapped`, so
            // arming must not resurrect a retry the live geometry overtook.
            let mut retry = super::MappedRequestRetry::default();
            retry.arm(Some(false), position, size, WindowState::Normal);
            assert_eq!(
                retry.take_on_mapped_event(&WindowEvent::Moved(PhysicalPosition::new(0, 0))),
                Some(armed(position, size, WindowState::Normal)),
            );
            retry.arm(None, newer.0, newer.1, WindowState::Normal);
            assert_eq!(
                retry.take_on_mapped_event(&WindowEvent::Moved(PhysicalPosition::new(0, 0))),
                None,
                "no re-arm is allowed once a mapped event has been seen"
            );
            let mut retry = super::MappedRequestRetry::default();
            retry.arm(Some(true), position, size, WindowState::Normal);
            assert_eq!(
                retry.take_on_mapped_event(&WindowEvent::Moved(PhysicalPosition::new(0, 0))),
                None,
                "a confirmed-visible window holds nothing"
            );
        }

        #[test]
        fn a_premap_state_is_reapplied_on_the_first_mapped_event() {
            use waterui::window::WindowState;
            use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
            use winit::event::WindowEvent;

            let position = LogicalPosition::new(12.0, 34.0);
            let size = LogicalSize::new(800.0, 300.0);

            // A Fullscreen request made while the window was still unmapped
            // is held and delivered once on the first mapped-signal event.
            for state in [
                WindowState::Fullscreen,
                WindowState::Minimized,
                WindowState::Closed,
            ] {
                let mut retry = super::MappedRequestRetry::default();
                retry.arm(Some(false), position, size, state);
                assert_eq!(
                    retry.take_on_mapped_event(&WindowEvent::Moved(PhysicalPosition::new(0, 0))),
                    Some(super::PendingMappedRequest {
                        position,
                        size,
                        state,
                    }),
                    "a pre-map {state:?} must be re-delivered on the first mapped event"
                );
                assert_eq!(
                    retry.take_on_mapped_event(&WindowEvent::Resized(PhysicalSize::new(1, 1))),
                    None,
                    "a pre-map {state:?} is delivered once only"
                );
            }
        }
    }
}

#[cfg(all(target_arch = "wasm32", feature = "web"))]
pub use web_impl::ExportedBrowserWindow as BrowserWindow;

#[cfg(feature = "winit")]
pub(crate) use winit_impl::ExportedWinitGpuContext as WinitGpuContext;

#[cfg(feature = "winit")]
pub use winit_impl::ExportedWinitWindow as WinitWindow;
