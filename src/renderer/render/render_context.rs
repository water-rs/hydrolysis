use super::{HydrolysisRenderer, TailMark};
use crate::renderer::HydroState;
use crate::renderer::navigation::{
    NavigationCapturedScene, NavigationTransitionFrame, draw_navigation_transition,
};
use waterui::navigation::{AnyNavigationTransition, NavigationTransitionDirection};
use waterui_backend_core::widget::NavigationMotion;
use waterui_core::Environment;
use waterui_core::layout::HorizontalAlignment;
use waterui_text::styled::StyledStr;

/// Render context passed to handlers.
#[derive(Debug, Clone, Copy)]
pub struct RenderContext {
    pub transform: cherenkov::kurbo::Affine,
    pub hit_transform: cherenkov::kurbo::Affine,
    pub bounds: cherenkov::kurbo::Rect,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HydrolysisWindowOrigin {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HydrolysisTextContextMenuMode {
    Overlay,
    NativeWindow,
}

pub(crate) struct WidgetRenderContext<'a> {
    renderer: &'a mut HydrolysisRenderer,
    pub transform: cherenkov::kurbo::Affine,
    pub hit_transform: cherenkov::kurbo::Affine,
    pub bounds: cherenkov::kurbo::Rect,
}

/// A theme recording that is appended to the frame scene when dropped.
pub(crate) struct ThemeDraw<'a> {
    recorder: cherenkov::Recorder,
    scene: &'a mut crate::scene::Scene,
    transform: cherenkov::kurbo::Affine,
}

impl<'a> ThemeDraw<'a> {
    /// Records into `scene` under `transform` when dropped.
    pub(crate) fn new(scene: &'a mut crate::scene::Scene, transform: cherenkov::kurbo::Affine) -> Self {
        Self {
            recorder: cherenkov::Recorder::new(),
            scene,
            transform,
        }
    }
}

impl core::ops::Deref for ThemeDraw<'_> {
    type Target = cherenkov::Recorder;

    fn deref(&self) -> &cherenkov::Recorder {
        &self.recorder
    }
}

impl core::ops::DerefMut for ThemeDraw<'_> {
    fn deref_mut(&mut self) -> &mut cherenkov::Recorder {
        &mut self.recorder
    }
}

impl Drop for ThemeDraw<'_> {
    fn drop(&mut self) {
        let picture = core::mem::take(&mut self.recorder).finish().into_picture();
        self.scene.draw_picture(self.transform, picture);
    }
}

/// An explicit offer from a native widget-owned content region.
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn bounded_proposal(bounds: cherenkov::kurbo::Rect) -> waterui_core::layout::ProposalSize {
    waterui_core::layout::ProposalSize::new(
        Some(bounds.width() as f32),
        Some(bounds.height() as f32),
    )
}

impl HydrolysisRenderer {
    /// Records theme drawing in `ctx`'s coordinate space into the frame scene.
    pub(crate) fn draw_context(&mut self, ctx: RenderContext) -> ThemeDraw<'_> {
        ThemeDraw::new(self.scene_mut(), ctx.transform)
    }
}

impl RenderContext {
    pub(crate) fn with_transforms(
        bounds: cherenkov::kurbo::Rect,
        transform: cherenkov::kurbo::Affine,
        hit_transform: cherenkov::kurbo::Affine,
    ) -> Self {
        Self {
            transform,
            hit_transform,
            bounds,
        }
    }

    #[must_use]
    pub fn child(&self, transform: cherenkov::kurbo::Affine, bounds: cherenkov::kurbo::Rect) -> Self {
        Self {
            transform: self.transform * transform,
            hit_transform: self.hit_transform * transform,
            bounds,
        }
    }

    #[must_use]
    pub(crate) fn with_identity_transforms(&self, bounds: cherenkov::kurbo::Rect) -> Self {
        Self {
            transform: cherenkov::kurbo::Affine::IDENTITY,
            hit_transform: cherenkov::kurbo::Affine::IDENTITY,
            bounds,
        }
    }
}

impl<'a> WidgetRenderContext<'a> {
    pub(crate) fn new(renderer: &'a mut HydrolysisRenderer, ctx: RenderContext) -> Self {
        Self {
            renderer,
            transform: ctx.transform,
            hit_transform: ctx.hit_transform,
            bounds: ctx.bounds,
        }
    }

    pub(crate) fn render_context(&self) -> RenderContext {
        RenderContext::with_transforms(self.bounds, self.transform, self.hit_transform)
    }

    /// The renderer-owned widget theme, cloned out as an `Rc` so callers can
    /// hold it without borrowing the context across a `&mut` renderer call.
    pub(crate) fn theme(&self) -> std::rc::Rc<dyn crate::engine::WidgetTheme> {
        self.renderer.theme()
    }

    pub(crate) fn child(
        &self,
        transform: cherenkov::kurbo::Affine,
        bounds: cherenkov::kurbo::Rect,
    ) -> RenderContext {
        self.render_context().child(transform, bounds)
    }

    pub(crate) fn renderer_mut(&mut self) -> &mut HydrolysisRenderer {
        self.renderer
    }

    /// Records theme drawing in this context's coordinate space; the
    /// recording lands in the frame scene when the guard drops.
    pub(crate) fn draw_context(&mut self) -> ThemeDraw<'_> {
        ThemeDraw::new(self.renderer.scene_mut(), self.transform)
    }

    pub(crate) fn state_mut(&mut self) -> &mut HydroState {
        &mut self.renderer.state
    }

    pub(crate) fn push_layer_rect(&mut self, alpha: f32, clip: cherenkov::kurbo::Rect) {
        self.renderer.push_layer_rect(alpha, self.transform, clip);
    }

    pub(crate) fn pop_layer(&mut self) {
        self.renderer.pop_layer();
    }

    pub(crate) fn render_styled_text(
        &mut self,
        styled: StyledStr,
        alignment: HorizontalAlignment,
        env: &Environment,
        bounds: cherenkov::kurbo::Rect,
    ) {
        self.render_styled_text_limited(styled, alignment, env, bounds, None);
    }

    pub(crate) fn render_styled_text_limited(
        &mut self,
        styled: StyledStr,
        alignment: HorizontalAlignment,
        env: &Environment,
        bounds: cherenkov::kurbo::Rect,
        max_lines: Option<usize>,
    ) {
        let child_ctx = self.child(
            cherenkov::kurbo::Affine::translate((bounds.x0, bounds.y0)),
            cherenkov::kurbo::Rect::new(0.0, 0.0, bounds.width(), bounds.height()),
        );
        let renderer = self.renderer_mut();
        let (state, scene) = renderer.state_and_scene_mut();
        HydrolysisRenderer::render_styled_text_limited(
            state,
            scene,
            child_ctx,
            styled,
            alignment,
            env,
            max_lines.map_or(TailMark::None, TailMark::Clip),
        );
    }

    pub(crate) fn render_styled_text_single_line_centered(
        &mut self,
        styled: StyledStr,
        env: &Environment,
        bounds: cherenkov::kurbo::Rect,
    ) {
        let child_ctx = self.child(
            cherenkov::kurbo::Affine::translate((bounds.x0, bounds.y0)),
            cherenkov::kurbo::Rect::new(0.0, 0.0, bounds.width(), bounds.height()),
        );
        let renderer = self.renderer_mut();
        let (state, scene) = renderer.state_and_scene_mut();
        HydrolysisRenderer::render_styled_text_single_line_centered(
            state, scene, child_ctx, styled, env,
        );
    }

    pub(crate) fn append_scene(&mut self, scene: &crate::scene::Scene) {
        self.renderer
            .scene_mut()
            .append(scene, Some(self.transform));
    }

    pub(crate) fn draw_navigation_transition(
        &mut self,
        style: AnyNavigationTransition,
        motion: NavigationMotion,
        direction: NavigationTransitionDirection,
        progress: f64,
        from_scene: &NavigationCapturedScene,
        to_scene: &NavigationCapturedScene,
    ) {
        draw_navigation_transition(NavigationTransitionFrame {
            scene: self.renderer.scene_mut(),
            transform: self.transform,
            bounds: self.bounds,
            style,
            motion,
            direction,
            progress,
            from_scene,
            to_scene,
        });
    }
}
