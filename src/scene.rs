use std::collections::HashMap;
use std::sync::Arc;

use vello::kurbo::{Affine, BezPath, Rect, Shape, Stroke};
use vello::peniko::{BlendMode, Brush, BrushRef, Color, Fill, ImageBrush, ImageBrushRef};
use waterui_graphics::{
    GlyphRun, HybridRenderer, HybridScene2D, HybridUpload, Scene2D, SceneRecording, VelloScene2D,
};

use crate::scene_renderer::RegisteredImage;

const FLATTEN_TOLERANCE: f64 = 0.1;

pub(crate) trait SceneTarget: Scene2D {
    fn draw_blurred_rounded_rect(
        &mut self,
        transform: Affine,
        rect: Rect,
        color: Color,
        radius: f64,
        std_dev: f64,
    );
}

#[derive(Clone, Debug, Default)]
pub struct WindowScene {
    parts: Vec<ScenePart>,
    open_layers: u32,
    has_content: bool,
}

#[derive(Clone, Debug)]
enum ScenePart {
    Drawing(Arc<SceneRecording>),
    Appended {
        scene: Arc<WindowScene>,
        transform: Affine,
    },
    BlurredRoundedRect {
        transform: Affine,
        rect: Rect,
        color: Color,
        radius: f64,
        std_dev: f64,
    },
}

impl WindowScene {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn has_content(&self) -> bool {
        self.has_content
    }

    #[must_use]
    pub(crate) fn open_layer_count(&self) -> u32 {
        self.open_layers
    }

    #[must_use]
    pub(crate) fn part_count(&self) -> usize {
        self.parts.len()
    }

    pub fn reset(&mut self) {
        self.parts.clear();
        self.open_layers = 0;
        self.has_content = false;
    }

    fn recording_mut(&mut self) -> &mut SceneRecording {
        let fresh = match self.parts.last_mut() {
            Some(ScenePart::Drawing(recording)) => Arc::get_mut(recording).is_none(),
            _ => true,
        };
        if fresh {
            self.parts
                .push(ScenePart::Drawing(Arc::new(SceneRecording::new())));
        }
        let Some(ScenePart::Drawing(recording)) = self.parts.last_mut() else {
            unreachable!("hydrolysis scene: a fresh recording part was just pushed")
        };
        Arc::get_mut(recording).expect("hydrolysis scene: a fresh recording is uniquely owned")
    }

    pub fn append(&mut self, scene: &WindowScene, transform: Option<Affine>) {
        self.has_content |= scene.has_content;
        self.open_layers = self
            .open_layers
            .checked_add(scene.open_layers)
            .expect("hydrolysis scene layer count overflow");
        self.parts.push(ScenePart::Appended {
            scene: Arc::new(scene.clone()),
            transform: transform.unwrap_or(Affine::IDENTITY),
        });
    }

    #[cfg(test)]
    fn force_open_layers(&mut self, count: u32) {
        self.open_layers = count;
    }

    pub(crate) fn replay(&self, target: &mut impl SceneTarget, transform: Affine) {
        for part in &self.parts {
            match part {
                ScenePart::Drawing(recording) => {
                    recording.replay(target, Some(transform));
                }
                ScenePart::Appended {
                    scene,
                    transform: appended,
                } => {
                    scene.replay(target, transform * *appended);
                }
                ScenePart::BlurredRoundedRect {
                    transform: part_transform,
                    rect,
                    color,
                    radius,
                    std_dev,
                } => {
                    target.draw_blurred_rounded_rect(
                        transform * *part_transform,
                        *rect,
                        *color,
                        *radius,
                        *std_dev,
                    );
                }
            }
        }
    }

    pub fn fill<'b>(
        &mut self,
        fill: Fill,
        transform: Affine,
        brush: impl Into<BrushRef<'b>>,
        brush_transform: Option<Affine>,
        shape: &impl Shape,
    ) {
        let brush = brush.into().to_owned();
        let path = shape.to_path(FLATTEN_TOLERANCE);
        Scene2D::fill(self, fill, transform, &brush, brush_transform, &path);
    }

    pub fn stroke<'b>(
        &mut self,
        style: &Stroke,
        transform: Affine,
        brush: impl Into<BrushRef<'b>>,
        brush_transform: Option<Affine>,
        shape: &impl Shape,
    ) {
        let brush = brush.into().to_owned();
        let path = shape.to_path(FLATTEN_TOLERANCE);
        Scene2D::stroke(self, style, transform, &brush, brush_transform, &path);
    }

    pub fn push_layer(
        &mut self,
        fill: Fill,
        blend: BlendMode,
        alpha: f32,
        transform: Affine,
        clip: &impl Shape,
    ) {
        let path = clip.to_path(FLATTEN_TOLERANCE);
        Scene2D::push_layer(self, fill, blend, alpha, transform, &path);
    }

    pub fn push_clip_layer(&mut self, fill: Fill, transform: Affine, clip: &impl Shape) {
        let path = clip.to_path(FLATTEN_TOLERANCE);
        Scene2D::push_clip_layer(self, fill, transform, &path);
    }

    pub fn pop_layer(&mut self) {
        Scene2D::pop_layer(self);
    }

    pub fn draw_image<'b>(&mut self, image: impl Into<ImageBrushRef<'b>>, transform: Affine) {
        let image = image.into().to_owned();
        Scene2D::draw_image(self, &image, transform);
    }

    pub fn draw_blurred_rounded_rect(
        &mut self,
        transform: Affine,
        rect: Rect,
        color: Color,
        radius: f64,
        std_dev: f64,
    ) {
        <Self as SceneTarget>::draw_blurred_rounded_rect(
            self, transform, rect, color, radius, std_dev,
        );
    }
}

impl Scene2D for WindowScene {
    fn fill(
        &mut self,
        fill: Fill,
        transform: Affine,
        brush: &Brush,
        brush_transform: Option<Affine>,
        shape: &BezPath,
    ) {
        self.has_content = true;
        self.recording_mut()
            .fill(fill, transform, brush, brush_transform, shape);
    }

    fn stroke(
        &mut self,
        stroke: &Stroke,
        transform: Affine,
        brush: &Brush,
        brush_transform: Option<Affine>,
        shape: &BezPath,
    ) {
        self.has_content = true;
        self.recording_mut()
            .stroke(stroke, transform, brush, brush_transform, shape);
    }

    fn push_layer(
        &mut self,
        fill: Fill,
        blend: BlendMode,
        alpha: f32,
        transform: Affine,
        clip: &BezPath,
    ) {
        self.open_layers = self
            .open_layers
            .checked_add(1)
            .expect("hydrolysis scene layer count overflow");
        self.recording_mut()
            .push_layer(fill, blend, alpha, transform, clip);
    }

    fn push_clip_layer(&mut self, fill: Fill, transform: Affine, clip: &BezPath) {
        self.open_layers = self
            .open_layers
            .checked_add(1)
            .expect("hydrolysis scene layer count overflow");
        self.recording_mut().push_clip_layer(fill, transform, clip);
    }

    fn pop_layer(&mut self) {
        self.open_layers = self
            .open_layers
            .checked_sub(1)
            .expect("hydrolysis scene layer stack underflow");
        self.recording_mut().pop_layer();
    }

    fn draw_image(&mut self, image: &ImageBrush, transform: Affine) {
        self.has_content = true;
        self.recording_mut().draw_image(image, transform);
    }

    fn draw_glyph_run(&mut self, run: &GlyphRun<'_>) {
        self.has_content |= !run.glyphs.is_empty();
        self.recording_mut().draw_glyph_run(run);
    }

    fn reset(&mut self) {
        WindowScene::reset(self);
    }
}

impl SceneTarget for WindowScene {
    fn draw_blurred_rounded_rect(
        &mut self,
        transform: Affine,
        rect: Rect,
        color: Color,
        radius: f64,
        std_dev: f64,
    ) {
        self.has_content = true;
        self.parts.push(ScenePart::BlurredRoundedRect {
            transform,
            rect,
            color,
            radius,
            std_dev,
        });
    }
}

pub(crate) struct ClassicTarget<'a> {
    scene: VelloScene2D<'a>,
}

impl<'a> ClassicTarget<'a> {
    pub(crate) const fn new(scene: &'a mut vello::Scene) -> Self {
        Self {
            scene: VelloScene2D::new(scene),
        }
    }
}

impl Scene2D for ClassicTarget<'_> {
    fn fill(
        &mut self,
        fill: Fill,
        transform: Affine,
        brush: &Brush,
        brush_transform: Option<Affine>,
        shape: &BezPath,
    ) {
        self.scene
            .fill(fill, transform, brush, brush_transform, shape);
    }

    fn stroke(
        &mut self,
        stroke: &Stroke,
        transform: Affine,
        brush: &Brush,
        brush_transform: Option<Affine>,
        shape: &BezPath,
    ) {
        self.scene
            .stroke(stroke, transform, brush, brush_transform, shape);
    }

    fn push_layer(
        &mut self,
        fill: Fill,
        blend: BlendMode,
        alpha: f32,
        transform: Affine,
        clip: &BezPath,
    ) {
        self.scene.push_layer(fill, blend, alpha, transform, clip);
    }

    fn push_clip_layer(&mut self, fill: Fill, transform: Affine, clip: &BezPath) {
        self.scene.push_clip_layer(fill, transform, clip);
    }

    fn pop_layer(&mut self) {
        self.scene.pop_layer();
    }

    fn draw_image(&mut self, image: &ImageBrush, transform: Affine) {
        self.scene.draw_image(image, transform);
    }

    fn draw_glyph_run(&mut self, run: &GlyphRun<'_>) {
        self.scene.draw_glyph_run(run);
    }

    fn reset(&mut self) {
        self.scene.reset();
    }
}

impl SceneTarget for ClassicTarget<'_> {
    fn draw_blurred_rounded_rect(
        &mut self,
        transform: Affine,
        rect: Rect,
        color: Color,
        radius: f64,
        std_dev: f64,
    ) {
        self.scene
            .scene_mut()
            .draw_blurred_rounded_rect(transform, rect, color, radius, std_dev);
    }
}

pub(crate) struct HybridTarget<'a> {
    scene: &'a mut vello_hybrid::Scene,
    renderer: &'a mut HybridRenderer,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    encoder: &'a mut wgpu::CommandEncoder,
    images: &'a HashMap<u64, RegisteredImage>,
    bindings: &'a mut vello_hybrid::TextureBindings,
}

impl<'a> HybridTarget<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        scene: &'a mut vello_hybrid::Scene,
        renderer: &'a mut HybridRenderer,
        device: &'a wgpu::Device,
        queue: &'a wgpu::Queue,
        encoder: &'a mut wgpu::CommandEncoder,
        images: &'a HashMap<u64, RegisteredImage>,
        bindings: &'a mut vello_hybrid::TextureBindings,
    ) -> Self {
        Self {
            scene,
            renderer,
            device,
            queue,
            encoder,
            images,
            bindings,
        }
    }

    fn scene2d(&mut self) -> HybridScene2D<'_> {
        HybridScene2D::new(
            self.scene,
            self.renderer,
            HybridUpload::new(self.device, self.queue, self.encoder),
        )
    }

    pub(crate) fn fill_background(&mut self, color: Color, width: u16, height: u16) {
        let state = self.scene.save_current_state();
        self.scene.set_transform(Affine::IDENTITY);
        self.scene.reset_paint_transform();
        self.scene.set_paint(color);
        self.scene
            .fill_rect(&Rect::new(0.0, 0.0, f64::from(width), f64::from(height)));
        self.scene.restore_state(state);
    }
}

impl Scene2D for HybridTarget<'_> {
    fn fill(
        &mut self,
        fill: Fill,
        transform: Affine,
        brush: &Brush,
        brush_transform: Option<Affine>,
        shape: &BezPath,
    ) {
        self.scene2d()
            .fill(fill, transform, brush, brush_transform, shape);
    }

    fn stroke(
        &mut self,
        stroke: &Stroke,
        transform: Affine,
        brush: &Brush,
        brush_transform: Option<Affine>,
        shape: &BezPath,
    ) {
        self.scene2d()
            .stroke(stroke, transform, brush, brush_transform, shape);
    }

    fn push_layer(
        &mut self,
        fill: Fill,
        blend: BlendMode,
        alpha: f32,
        transform: Affine,
        clip: &BezPath,
    ) {
        self.scene2d()
            .push_layer(fill, blend, alpha, transform, clip);
    }

    fn push_clip_layer(&mut self, fill: Fill, transform: Affine, clip: &BezPath) {
        self.scene2d().push_clip_layer(fill, transform, clip);
    }

    fn pop_layer(&mut self) {
        self.scene2d().pop_layer();
    }

    fn draw_image(&mut self, image: &ImageBrush, transform: Affine) {
        let Some(registered) = self.images.get(&image.image.data.id()) else {
            self.scene2d().draw_image(image, transform);
            return;
        };
        assert_eq!(
            image.sampler.x_extend,
            vello::peniko::Extend::Pad,
            "hydrolysis hybrid image draw: registered textures only support pad extend"
        );
        assert_eq!(
            image.sampler.y_extend,
            vello::peniko::Extend::Pad,
            "hydrolysis hybrid image draw: registered textures only support pad extend"
        );
        self.bindings
            .insert(registered.texture_id, registered.view.clone());
        let state = self.scene.save_current_state();
        self.scene.set_transform(transform);
        if image.sampler.alpha != 1.0 {
            self.scene
                .push_layer(None, None, Some(image.sampler.alpha), None, None);
        }
        self.scene.draw_texture_rects(
            registered.texture_id,
            image.sampler.quality,
            [vello_hybrid::SampleRect {
                source_region: vello_common::geometry::RectU16::new(
                    0,
                    0,
                    u16::try_from(registered.texture.width())
                        .expect("hydrolysis registered texture width exceeds u16"),
                    u16::try_from(registered.texture.height())
                        .expect("hydrolysis registered texture height exceeds u16"),
                ),
                transform: Affine::IDENTITY,
            }],
        );
        if image.sampler.alpha != 1.0 {
            self.scene.pop_layer();
        }
        self.scene.restore_state(state);
    }

    fn draw_glyph_run(&mut self, run: &GlyphRun<'_>) {
        self.scene2d().draw_glyph_run(run);
    }

    fn reset(&mut self) {
        self.scene2d().reset();
    }
}

impl SceneTarget for HybridTarget<'_> {
    fn draw_blurred_rounded_rect(
        &mut self,
        transform: Affine,
        rect: Rect,
        color: Color,
        radius: f64,
        std_dev: f64,
    ) {
        let state = self.scene.save_current_state();
        self.scene.set_transform(transform);
        self.scene.reset_paint_transform();
        self.scene.set_paint(color);
        #[allow(clippy::cast_possible_truncation)]
        self.scene
            .fill_blurred_rounded_rect(&rect, radius as f32, std_dev as f32, false);
        self.scene.restore_state(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vello::peniko::FontData;

    #[derive(Debug, PartialEq)]
    enum SpyEvent {
        Fill(Affine),
        Stroke(Affine),
        PushLayer {
            alpha: f32,
            transform: Affine,
        },
        PushClipLayer(Affine),
        PopLayer,
        DrawImage(Affine),
        GlyphRun(usize),
        Blur {
            transform: Affine,
            rect: Rect,
            radius: f64,
            std_dev: f64,
        },
    }

    #[derive(Default)]
    struct SpyTarget {
        events: Vec<SpyEvent>,
    }

    impl Scene2D for SpyTarget {
        fn fill(
            &mut self,
            _fill: Fill,
            transform: Affine,
            _brush: &Brush,
            _brush_transform: Option<Affine>,
            _shape: &BezPath,
        ) {
            self.events.push(SpyEvent::Fill(transform));
        }

        fn stroke(
            &mut self,
            _stroke: &Stroke,
            transform: Affine,
            _brush: &Brush,
            _brush_transform: Option<Affine>,
            _shape: &BezPath,
        ) {
            self.events.push(SpyEvent::Stroke(transform));
        }

        fn push_layer(
            &mut self,
            _fill: Fill,
            _blend: BlendMode,
            alpha: f32,
            transform: Affine,
            _clip: &BezPath,
        ) {
            self.events.push(SpyEvent::PushLayer { alpha, transform });
        }

        fn push_clip_layer(&mut self, _fill: Fill, transform: Affine, _clip: &BezPath) {
            self.events.push(SpyEvent::PushClipLayer(transform));
        }

        fn pop_layer(&mut self) {
            self.events.push(SpyEvent::PopLayer);
        }

        fn draw_image(&mut self, _image: &ImageBrush, transform: Affine) {
            self.events.push(SpyEvent::DrawImage(transform));
        }

        fn draw_glyph_run(&mut self, run: &GlyphRun<'_>) {
            self.events.push(SpyEvent::GlyphRun(run.glyphs.len()));
        }

        fn reset(&mut self) {
            self.events.clear();
        }
    }

    impl SceneTarget for SpyTarget {
        fn draw_blurred_rounded_rect(
            &mut self,
            transform: Affine,
            rect: Rect,
            _color: Color,
            radius: f64,
            std_dev: f64,
        ) {
            self.events.push(SpyEvent::Blur {
                transform,
                rect,
                radius,
                std_dev,
            });
        }
    }

    fn test_font() -> FontData {
        FontData::new(
            vello::peniko::Blob::new(Arc::new(include_bytes!("../test-fonts/Roboto-Regular.ttf"))),
            0,
        )
    }

    fn glyph_run<'a>(
        font: &'a FontData,
        brush: &'a Brush,
        glyphs: &'a [waterui_graphics::Glyph],
    ) -> GlyphRun<'a> {
        GlyphRun {
            font,
            font_size: 16.0,
            normalized_coords: &[],
            transform: Affine::IDENTITY,
            brush,
            brush_alpha: 1.0,
            style: Fill::NonZero.into(),
            glyphs,
        }
    }

    fn fill_rect(scene: &mut WindowScene, transform: Affine) {
        scene.fill(
            Fill::NonZero,
            transform,
            Color::BLACK,
            None,
            &Rect::new(0.0, 0.0, 4.0, 4.0),
        );
    }

    #[test]
    fn append_replays_nested_transforms_in_order() {
        let mut child = WindowScene::new();
        fill_rect(&mut child, Affine::translate((10.0, 0.0)));

        let mut parent = WindowScene::new();
        parent.append(&child, Some(Affine::scale(2.0)));

        let mut spy = SpyTarget::default();
        parent.replay(&mut spy, Affine::translate((0.0, 5.0)));

        assert_eq!(
            spy.events,
            [SpyEvent::Fill(
                Affine::translate((0.0, 5.0)) * Affine::scale(2.0) * Affine::translate((10.0, 0.0))
            )]
        );
    }

    #[test]
    fn appended_snapshot_is_unchanged_when_source_extends_or_resets() {
        let mut child = WindowScene::new();
        fill_rect(&mut child, Affine::translate((1.0, 0.0)));

        let mut parent = WindowScene::new();
        parent.append(&child, None);

        fill_rect(&mut child, Affine::translate((2.0, 0.0)));
        let mut spy = SpyTarget::default();
        parent.replay(&mut spy, Affine::IDENTITY);
        assert_eq!(spy.events, [SpyEvent::Fill(Affine::translate((1.0, 0.0)))]);

        child.reset();
        let mut spy = SpyTarget::default();
        parent.replay(&mut spy, Affine::IDENTITY);
        assert_eq!(spy.events, [SpyEvent::Fill(Affine::translate((1.0, 0.0)))]);
    }

    #[test]
    fn text_only_scene_reports_content() {
        let font = test_font();
        let brush = Brush::Solid(Color::BLACK);
        let glyphs = [waterui_graphics::Glyph {
            id: 3,
            x: 0.0,
            y: 0.0,
        }];

        let mut scene = WindowScene::new();
        assert!(!scene.has_content());
        Scene2D::draw_glyph_run(&mut scene, &glyph_run(&font, &brush, &glyphs));
        assert!(scene.has_content());

        let mut spy = SpyTarget::default();
        scene.replay(&mut spy, Affine::IDENTITY);
        assert_eq!(spy.events, [SpyEvent::GlyphRun(1)]);
    }

    #[test]
    fn empty_glyph_run_reports_no_content() {
        let font = test_font();
        let brush = Brush::Solid(Color::BLACK);

        let mut scene = WindowScene::new();
        Scene2D::draw_glyph_run(&mut scene, &glyph_run(&font, &brush, &[]));
        assert!(!scene.has_content());
    }

    #[test]
    fn clip_only_scene_reports_no_content() {
        let mut scene = WindowScene::new();
        scene.push_clip_layer(
            Fill::NonZero,
            Affine::IDENTITY,
            &Rect::new(0.0, 0.0, 4.0, 4.0),
        );
        assert!(!scene.has_content());
        assert_eq!(scene.open_layer_count(), 1);
        scene.pop_layer();
        assert!(!scene.has_content());
        assert_eq!(scene.open_layer_count(), 0);
    }

    #[test]
    fn open_layer_counts_survive_append_and_reset() {
        let mut child = WindowScene::new();
        child.push_clip_layer(
            Fill::NonZero,
            Affine::IDENTITY,
            &Rect::new(0.0, 0.0, 4.0, 4.0),
        );
        assert_eq!(child.open_layer_count(), 1);

        let mut parent = WindowScene::new();
        parent.push_clip_layer(
            Fill::NonZero,
            Affine::IDENTITY,
            &Rect::new(0.0, 0.0, 8.0, 8.0),
        );
        parent.append(&child, None);
        assert_eq!(parent.open_layer_count(), 2);

        let mut spy = SpyTarget::default();
        parent.replay(&mut spy, Affine::IDENTITY);
        assert_eq!(
            spy.events,
            [
                SpyEvent::PushClipLayer(Affine::IDENTITY),
                SpyEvent::PushClipLayer(Affine::IDENTITY),
            ]
        );

        parent.reset();
        assert_eq!(parent.open_layer_count(), 0);
        assert_eq!(parent.part_count(), 0);
        assert!(!parent.has_content());
    }

    #[test]
    #[should_panic(expected = "hydrolysis scene layer stack underflow")]
    fn pop_layer_underflow_panics() {
        let mut scene = WindowScene::new();
        scene.pop_layer();
    }

    #[test]
    #[should_panic(expected = "hydrolysis scene layer count overflow")]
    fn push_layer_overflow_panics() {
        let mut scene = WindowScene::new();
        scene.force_open_layers(u32::MAX);
        scene.push_clip_layer(
            Fill::NonZero,
            Affine::IDENTITY,
            &Rect::new(0.0, 0.0, 4.0, 4.0),
        );
    }

    #[test]
    #[should_panic(expected = "hydrolysis scene layer count overflow")]
    fn append_layer_count_overflow_panics() {
        let mut child = WindowScene::new();
        child.force_open_layers(u32::MAX);
        let mut parent = WindowScene::new();
        parent.push_clip_layer(
            Fill::NonZero,
            Affine::IDENTITY,
            &Rect::new(0.0, 0.0, 4.0, 4.0),
        );
        parent.append(&child, None);
    }

    #[test]
    fn blurred_rounded_rect_replays_in_recorded_order() {
        let mut scene = WindowScene::new();
        fill_rect(&mut scene, Affine::translate((1.0, 0.0)));
        scene.draw_blurred_rounded_rect(
            Affine::translate((2.0, 0.0)),
            Rect::new(0.0, 0.0, 8.0, 8.0),
            Color::BLACK,
            4.0,
            2.0,
        );
        fill_rect(&mut scene, Affine::translate((3.0, 0.0)));

        let mut spy = SpyTarget::default();
        scene.replay(&mut spy, Affine::IDENTITY);
        assert_eq!(
            spy.events,
            [
                SpyEvent::Fill(Affine::translate((1.0, 0.0))),
                SpyEvent::Blur {
                    transform: Affine::translate((2.0, 0.0)),
                    rect: Rect::new(0.0, 0.0, 8.0, 8.0),
                    radius: 4.0,
                    std_dev: 2.0,
                },
                SpyEvent::Fill(Affine::translate((3.0, 0.0))),
            ]
        );
    }

    #[test]
    fn draw_image_marks_content_and_replays() {
        let image = vello::peniko::ImageBrush::new(vello::peniko::ImageData {
            data: vello::peniko::Blob::new(Arc::new(vec![255_u8; 16])),
            format: vello::peniko::ImageFormat::Rgba8,
            alpha_type: vello::peniko::ImageAlphaType::Alpha,
            width: 2,
            height: 2,
        });
        let mut scene = WindowScene::new();
        scene.draw_image(&image, Affine::translate((7.0, 0.0)));
        assert!(scene.has_content());

        let mut spy = SpyTarget::default();
        scene.replay(&mut spy, Affine::IDENTITY);
        assert_eq!(
            spy.events,
            [SpyEvent::DrawImage(Affine::translate((7.0, 0.0)))]
        );
    }
}
