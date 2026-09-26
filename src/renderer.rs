//! The hydrolysis renderer.
//!
//! This module owns [`HydrolysisRenderer`] — its construction and field
//! layout live here; behavior is split into focused submodules:
//!
//! - [`dispatch`]: type-erased view dispatch and `HydroNativeView` registration
//! - [`frame`]: frame lifecycle, layer stack, frame triggers, statistics
//! - [`FrameSignals`]: the shared frame trigger handle (lives in
//!   `waterui-backend-core`, shared with other self-drawn backends)
//! - [`retained`]: retained scene — replayable draws, `Dynamic` placements,
//!   reactive patching, scroll caches, window-frame capture/replay
//! - [`signals`]: signal watching and animated-value sampling
//! - [`views`] / [`metadata`]: raw view and metadata handlers
//! - [`bindings`]: hit-test/gesture/text-input/scroll bindings and queries
//! - [`input`] / [`lifecycle`] / [`navigation`] / [`accessibility`] /
//!   [`render`]: interaction, lifecycle, navigation, a11y, and measurement
//!   subsystems

#[cfg(feature = "accessibility")]
mod accessibility;
mod bindings;
mod frame;
mod identity;
mod input;
mod interaction_layers;
mod lifecycle;
mod metadata;
mod native_measure;
mod navigation;
mod render;
mod retained;
mod signals;
#[cfg(test)]
pub(crate) mod tests;
mod tree;
mod views;

pub(crate) use frame::*;
#[cfg(feature = "frame-profile")]
pub use frame::FrameStageTimes;
pub(crate) use identity::*;
pub(crate) use native_measure::*;
pub(crate) use retained::*;
pub(crate) use tree::*;
pub(crate) use views::*;
pub(crate) use waterui_backend_core::frame_signals::FrameSignals;

#[cfg(feature = "accessibility")]
use accessibility::*;
use core::f64::consts::TAU;
use core::time::Duration;
pub(crate) use input::*;
pub(crate) use interaction_layers::*;
pub(crate) use lifecycle::lazy;
pub(crate) use lifecycle::*;
pub(crate) use navigation::*;
pub(crate) use render::WidgetRenderContext;
pub(crate) use render::*;
pub(crate) use render::{
    anchor_point, circle_arc_path, estimate_layout_intrinsic, gesture_group_identity,
    normalize_layout_view, normalize_view_for_render, resolved_morph_shape_to_path,
    resolved_shape_to_path, transformed_rect, unit_path_to_path,
};
use rustc_hash::FxHashSet;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};

#[cfg(feature = "accessibility")]
use accesskit::{
    Action as AccessibilityAction, ActionData as AccessibilityActionData,
    ActionRequest as AccessibilityActionRequest, Node as AccessibilityNode,
    NodeId as AccessibilityNodeId, Rect as AccessibilityRect, Role as AccessibilityNodeRole,
    TextDirection as AccessibilityTextDirection, Toggled as AccessibilityToggled,
    TreeId as AccessibilityTreeId, TreeInfo as AccessibilityTree,
    TreeUpdate as AccessibilityTreeUpdate,
};
use executor_core::spawn_local;
use nami::{Binding, Signal};
use std::sync::Arc;
use waterkit_clipboard::Clipboard;
use waterui::ViewExt;
#[cfg(feature = "accessibility")]
use waterui::accessibility::AccessibilityValue;
use waterui::accessibility::{
    AccessibilityChildren, AccessibilityHidden, AccessibilityIdentifier, AccessibilityLabel,
    AccessibilityRole, AccessibilityState, AccessibilityStateSignal,
};
use waterui::animation::{Animation, Curve};
use waterui::background::{Background, MaterialBackground};
use waterui::border::Border;
use waterui::component::badge::BadgeConfig;
use waterui::component::focus::Focused;
use waterui::component::list::{ListConfig, ListItem};
use waterui::component::progress::{ProgressConfig, ProgressStyle};
use waterui::component::table::{TableColumn, TableConfig};
use waterui::cursor::{Cursor, CursorStyle};
use waterui::drag_drop::{Draggable, DropDestination};
use waterui::filter::Opacity;
use waterui::gesture::{Gesture, GestureObserver};
use waterui::interaction::Hittable;
use waterui::metadata::context_menu::{ContextMenu, ResolvedContextMenu};
use waterui::metadata::secure::{HighDynamicRange, Secure, StandardDynamicRange};
use waterui::navigation::tab::{NativeTabStyle, TabsLayout};
use waterui::navigation::{
    CustomNavigationController, NavigationController, NavigationSplitLayout, NavigationStack,
    NavigationToolbarPlacement, NavigationTransaction, NavigationTransitionDestination,
    NavigationTransitionSource, NavigationView,
};
use waterui::style::{Offset, Rotation, Scale, Shadow};
use waterui::theme;
use waterui::widget::Divider;
use waterui::window::{Window, WindowState, WindowStyle};
use waterui_controls::button::{Button, ButtonConfig, ButtonStyle};
use waterui_controls::label::Label as SemanticLabel;
use waterui_controls::menu::{ResolvedCommand, ResolvedMenu, ResolvedMenuItem};
use waterui_controls::slider::SliderConfig;
use waterui_controls::stepper::StepperConfig;
use waterui_controls::text_field::{ResolvedTextFieldConfig, TextField};
use waterui_controls::toggle::ToggleConfig;
use waterui_core::dynamic::{Dynamic, DynamicInitialContent};
use waterui_core::event::{Event, HoverEvent, LifeCycle, LifeCycleHook, OnEvent};
use waterui_core::handler::{AnyViewBuilder, BoxedAction, SharedAction};
use waterui_core::layout::{
    HorizontalAlignment, Layout, PlacedSubview, Point as LayoutPoint, ProposalSize,
    Rect as LayoutRect, Size as LayoutSize, StretchAxis, SubView, VerticalAlignment,
    ViewDimensions,
};
#[cfg(feature = "accessibility")]
use waterui_core::metadata::MetadataKey;
use waterui_core::view::Hook;
use waterui_core::views::Views;
use waterui_core::{
    AnyView, Environment, IgnorableMetadata, Metadata, Native, Retain, Str, View, impl_extractor,
};
use waterui_form::picker::PickerConfig;
use waterui_form::picker::color::ColorPickerConfig;
use waterui_form::picker::date::DatePickerConfig;
use waterui_form::secure::{Secure as FormSecure, SecureFieldConfig};
use waterui_graphics::color::{Color, WorkingColor};

use waterui_graphics::{FilteredView, Gradient, GpuContentView, SceneView, ShaderPaintView};

use waterui_icon::SystemIcon;
use waterui_layout::container::{FixedContainer, LazyContainer};
use waterui_layout::safe_area::IgnoreSafeArea;
#[cfg(feature = "accessibility")]
use waterui_layout::scroll::Axis as ScrollAxis;
use waterui_layout::scroll::ScrollView;
use waterui_layout::spacer::Spacer;
use waterui_map::MapConfig;
use waterui_shape::{ClipShape, ResolvedMorphShape, ResolvedShape, ShapeKind};
use waterui_text::font::FontWeight as TextFontWeight;
use waterui_text::styled::{Style as TextStyle, StyledStr};
use waterui_text::{Text, TextConfig};
use waterui_webview::WebView;

use crate::animation::{AnimatedScalarHandle, AnimationController, AnimationKey};
use crate::engine::{
    RadioIndicatorState, RadioSelectionMotion, TextCaretMotion, TextContextMenuMetrics,
};
use crate::gesture::GestureEngine;
use crate::platform::{
    KeyCode, Modifiers, PointerButton, PointerKind, TextInputPurpose, TextInputState, TouchPhase,
};
#[cfg(feature = "accessibility")]
use crate::scroll::ScrollHandle;
use crate::time::Instant;
use crate::widgets::inset_rect;

#[derive(Clone, Copy)]
pub(crate) struct DynamicRangePreference(pub(crate) bool);

const OPACITY_ANIMATION_KEY: usize = 0x0100_0001;
const SCALE_X_ANIMATION_KEY: usize = 0x0100_0002;
const SCALE_Y_ANIMATION_KEY: usize = 0x0100_0003;
const ROTATION_ANIMATION_KEY: usize = 0x0100_0004;
const OFFSET_X_ANIMATION_KEY: usize = 0x0100_0005;
const OFFSET_Y_ANIMATION_KEY: usize = 0x0100_0006;
const MORPH_PROGRESS_ANIMATION_KEY: usize = 0x0100_0007;

#[cfg(feature = "accessibility")]
pub(crate) use accessibility::{
    AccessibilityActionTarget, AccessibilityActivation, accessibility_container_child_environment,
    slider_step_for_range,
};
pub(crate) use input::{
    TextInputModel, TextInputTargetRegistration, TextSelectionSlot, clamp_to_char_boundary,
    text_editing,
};

/// The GPU-free dispatch core: everything the retained view tree's build,
/// patch and accessibility emission need, without a device, a scene, or a
/// style.
///
/// [`HydrolysisRenderer`] owns one and dereferences to it, so rendering code
/// reaches this state transparently while [`crate::SemanticRuntime`] can hold
/// just the core and prove by type that build, patch and semantic emission
/// can never touch the GPU. Methods that only use this state live in
/// `impl SemanticCore` blocks; methods that also touch rendering state stay
/// on `HydrolysisRenderer`.
///
/// `pub` only because `HydrolysisRenderer` dereferences to it — every member
/// is crate-internal.
pub struct SemanticCore {
    state: HydroState,
    hit_test: HitTestState,
    gesture_engine: GestureEngine,
    gesture_group_ids: BTreeMap<usize, usize>,
    next_gesture_group_id: usize,
    text_editing: TextEditingState,
    popup_menu: PopupMenuState,
    render_depth: usize,
    /// The retained nodes whose subtrees are currently flushing, innermost
    /// last — the ancestry chain input registration reads to tell a gesture
    /// registered inside a view from one attached to the view itself. The
    /// accessibility builder keeps its own copy for semantic-key identity; the
    /// input side additionally gets the pushes that mark where a view begins
    /// (a sub-view's root) without entering the a11y chain.
    owner_stack: Vec<RetainedIdentity>,
    /// Frame triggers shared with reactive closures; see [`FrameSignals`].
    signals: FrameSignals,
    lifecycle: LifecycleState,
    animation_controller: AnimationController,
    frame_instant: Instant,
    pub(crate) lazy: LazyState,
    pub(crate) navigation: NavigationState,
    #[cfg(feature = "accessibility")]
    accessibility: AccessibilityBuilder,
    /// The persistent window render tree (`tree::RenderNode`), built on a structural
    /// rebuild and re-flushed each frame. `None` before the first build.
    render_tree: Option<RenderNode>,
    /// Set when a widget-owned [`RetainedSubview`] applied a structural patch
    /// (a `Dynamic` swap or a collection membership reconcile) during a flush.
    /// The subview patch runs mid-flush — after the window pump's structural
    /// bookkeeping window — so the flag carries the change into the next refresh
    /// frame, which then runs the full animation-slot / measurement-cache prune
    /// cycle for the dropped subtrees.
    subview_structural_change: bool,
    /// Physical codes of key presses the IME consumed while it owned input.
    /// Their releases must be swallowed too — wl_keyboard delivers the release
    /// of an IME-consumed press in a later batch, after the commit that ended
    /// the composition.
    ime_swallowed_codes: Vec<keyboard_types::Code>,
    /// `true` when this core is the semantic walk — it emits no pointer
    /// machinery, so focus liveness may fall back to the semantic focus
    /// link. The semantic runner marks it at construction; a rendered
    /// runtime never does, so a rendered frame with no pointer targets
    /// still applies the rendered-runtime rule.
    #[cfg(feature = "accessibility")]
    semantic_walk: bool,
}

/// Core hydrolysis renderer state: a [`SemanticCore`] plus the Cherenkov
/// engine, the frame's scene recording and the surface state the
/// layout/encode pass needs.
pub struct HydrolysisRenderer {
    core: SemanticCore,
    /// The widget theme the runtime's style supplies to layout and encode.
    /// Never installed into the environment: build and patch cannot reach it.
    theme: Rc<dyn crate::engine::WidgetTheme>,
    /// The engine every surface this renderer draws on was created from; owns
    /// the device and the fonts and images the frames name.
    engine: Rc<cherenkov::Engine<cherenkov_gpu::Gpu>>,
    /// Engine font handles by parley font identity.
    scene: crate::scene::Scene,
    transient_scene: Option<crate::scene::Scene>,
    /// The clip layers open in `scene`, outermost first.
    active_scene_layers: Vec<ActiveSceneLayer>,
    /// Whole-scene pictures recorded this frame and not yet shown: one per
    /// `flush_scene_layer`, in order.
    frame_pictures: Vec<cherenkov::Picture>,
    /// Whether the scene was re-recorded since the last presented frame; a
    /// redraw without one keeps the root layer's retained picture.
    frame_recorded: bool,
    /// The layer above the scene that shows the per-frame text-input overlay
    /// (caret and selection), created on the first frame that needs it.
    overlay_layer: Option<cherenkov::Layer>,
    /// How many segments the last taken frame picture was assembled from.
    frame_picture_count: u32,
    window_bounds: cherenkov::kurbo::Rect,
    /// The transform the window's root content is flushed under: logical layout
    /// units onto the target's physical pixel grid.
    window_root_transform: cherenkov::kurbo::Affine,
    frame_clip_layers: u32,
    frame_max_clip_depth: u32,
    navigation_captures: Vec<NavigationSceneCapture>,
    /// CPU stage times accumulated by `flush_window_tree`, plus the GPU span
    /// the engine reports; drained per pump by `take_frame_stage_times`.
    /// `pub(crate)` so the runner's readback timing can add its stage in.
    #[cfg(feature = "frame-profile")]
    pub(crate) frame_stage_times: FrameStageTimes,
    /// Digest of the last layout pass's placed bounds; the frame-profile
    /// example compares it across runs to prove a change left layout output
    /// byte-identical.
    #[cfg(feature = "frame-profile")]
    last_layout_signature: Option<u64>,
}

impl HydrolysisRenderer {
    /// The engine's resource minter, for content that records fonts, images
    /// and shaders against the engine this renderer draws with.
    pub(crate) fn engine_resources(&self) -> Rc<cherenkov::Engine<cherenkov_gpu::Gpu>> {
        Rc::clone(&self.engine)
    }
}

impl core::ops::Deref for HydrolysisRenderer {
    type Target = SemanticCore;
    fn deref(&self) -> &SemanticCore {
        &self.core
    }
}

impl core::ops::DerefMut for HydrolysisRenderer {
    fn deref_mut(&mut self) -> &mut SemanticCore {
        &mut self.core
    }
}

const HIT_TEST_ALPHA_THRESHOLD: f32 = 0.01;

const TEXT_SELECTION_MULTI_CLICK_INTERVAL: Duration = Duration::from_millis(500);
const TEXT_SELECTION_MULTI_CLICK_DISTANCE: f64 = 6.0;
const TEXT_CONTEXT_MENU_WINDOW_TITLE: &str = "";

impl SemanticCore {
    pub(crate) fn new(frame_instant: Instant, engine: Option<Rc<crate::platform::Engine>>) -> Self {
        Self {
            state: HydroState::new(engine),
            hit_test: HitTestState::default(),
            gesture_engine: GestureEngine::default(),
            gesture_group_ids: BTreeMap::new(),
            next_gesture_group_id: 0,
            text_editing: TextEditingState::default(),
            popup_menu: PopupMenuState::default(),
            render_depth: 0,
            owner_stack: Vec::new(),
            signals: FrameSignals::new(frame_instant),
            lifecycle: LifecycleState::default(),
            animation_controller: AnimationController::default(),
            frame_instant,
            lazy: LazyState::default(),
            navigation: NavigationState::default(),
            #[cfg(feature = "accessibility")]
            accessibility: AccessibilityBuilder::default(),
            render_tree: None,
            subview_structural_change: false,
            ime_swallowed_codes: Vec::new(),
            #[cfg(feature = "accessibility")]
            semantic_walk: false,
        }
    }

    /// Runs `f` with accessibility-node registration suppressed. For a control
    /// whose own node already carries an internal sub-view's semantics (a merged
    /// label, a numeric value): emitting that sub-view inside this scope keeps it
    /// visual-only, so the control stays a single accessibility node instead of
    /// double-exposing its label as a separate node.
    #[cfg(feature = "accessibility")]
    pub(crate) fn with_suppressed_accessibility<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        #[cfg(feature = "accessibility")]
        self.push_accessibility_suppression();
        let result = f(self);
        #[cfg(feature = "accessibility")]
        self.pop_accessibility_suppression();
        result
    }

    /// The modifier snapshot the window last reported — pointer targets read
    /// it at commit time because pointer events carry no modifier state of
    /// their own (toggle and Shift range selection).
    pub(crate) fn modifiers(&self) -> Modifiers {
        self.hit_test.modifiers
    }

    /// Records that a widget-owned sub-view applied a structural patch during
    /// this flush, and requests a refresh frame so the change's prune cycle (and
    /// any layout it invalidated in ancestors) settles on the next pump.
    pub(crate) fn note_subview_structural_change(&mut self) {
        self.subview_structural_change = true;
        self.signals.request_refresh();
    }

    /// Consumes the carried subview structural-change flag; the refresh pump
    /// folds it into this frame's `structural_change` so the prune cycle runs.
    pub(crate) fn take_subview_structural_change(&mut self) -> bool {
        core::mem::take(&mut self.subview_structural_change)
    }
}

impl HydrolysisRenderer {
    /// A renderer drawing on surfaces of `engine` with `theme`.
    #[must_use]
    pub fn new(
        engine: Rc<cherenkov::Engine<cherenkov_gpu::Gpu>>,
        theme: Rc<dyn crate::engine::WidgetTheme>,
    ) -> Self {
        let frame_instant = Instant::now();
        Self {
            core: SemanticCore::new(frame_instant, Some(Rc::clone(&engine))),
            theme,
            engine,
            scene: crate::scene::Scene::new(),
            transient_scene: None,
            active_scene_layers: Vec::new(),
            frame_pictures: Vec::new(),
            frame_recorded: false,
            overlay_layer: None,
            frame_picture_count: 0,
            window_bounds: cherenkov::kurbo::Rect::ZERO,
            window_root_transform: cherenkov::kurbo::Affine::IDENTITY,
            frame_clip_layers: 0,
            frame_max_clip_depth: 0,
            navigation_captures: Vec::new(),
            #[cfg(feature = "frame-profile")]
            frame_stage_times: FrameStageTimes::default(),
            #[cfg(feature = "frame-profile")]
            last_layout_signature: None,
        }
    }

    /// The engine this renderer's surfaces belong to.
    #[must_use]
    pub fn engine(&self) -> &Rc<cherenkov::Engine<cherenkov_gpu::Gpu>> {
        &self.engine
    }

    /// The widget theme layout and encode draw with. Returned as a cloned
    /// `Rc` so callers may hold it across further `&mut self` calls.
    pub(crate) fn theme(&self) -> Rc<dyn crate::engine::WidgetTheme> {
        Rc::clone(&self.theme)
    }

    /// Runs `f` with accessibility-node registration suppressed. For a control
    /// whose own node already carries an internal sub-view's semantics (a merged
    /// label, a numeric value): flushing that sub-view inside this scope keeps it
    /// visual-only, so the control stays a single accessibility node instead of
    /// double-exposing its label as a separate node. Compiles to a plain call
    /// without the `accessibility` feature, so call sites need no gating.
    ///
    /// This shadows [`SemanticCore::with_suppressed_accessibility`] so rendered
    /// callers flush through a `&mut HydrolysisRenderer`; the core method is the
    /// one the semantic runtime's emission walk uses.
    pub(crate) fn with_suppressed_accessibility<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        #[cfg(feature = "accessibility")]
        self.push_accessibility_suppression();
        let result = f(self);
        #[cfg(feature = "accessibility")]
        self.pop_accessibility_suppression();
        result
    }
}

pub use render::HydroState;
use render::HydroSubview;
pub use render::RenderContext;
pub(crate) use render::ThemeDraw;
pub(crate) use render::{HydrolysisTextContextMenuMode, HydrolysisWindowOrigin};
