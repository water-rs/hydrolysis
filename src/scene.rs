//! The frame's drawing: an ordered list of Cherenkov commands recorded while
//! the retained tree is flushed, lowered into a [`Picture`] the root layer
//! shows.
//!
//! Hydrolysis draws whole-scene frames: every flush records the entire window
//! into one [`Scene`], and the surface's root layer takes it as its content.
//! The recording names engine resources — fonts and images — by id only, so a
//! scene is `Send` and cacheable; whoever records keeps the handles alive for
//! as long as a frame can show the recording.

use cherenkov::kurbo::{Affine, BezPath, Point, Rect, Shape as KurboShape, Stroke, Vec2};
use cherenkov::{
    Draw, EvenOdd, FontId, Glyph, GlyphRun, GlyphStyle, Group, ImageId, Paint, Picture,
    Sampling, Shadow, ShapeData, StaticRecorder, WorkingColor,
};

/// The fill rule a shape is filled with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fill {
    /// Non-zero winding.
    #[default]
    NonZero,
    /// Even-odd winding.
    EvenOdd,
}

/// A paint, or something that becomes one.
pub trait IntoPaint {
    /// The paint.
    fn into_paint(self) -> Paint;
}

impl IntoPaint for Paint {
    fn into_paint(self) -> Paint {
        self
    }
}

impl IntoPaint for &Paint {
    fn into_paint(self) -> Paint {
        self.clone()
    }
}

impl IntoPaint for WorkingColor {
    fn into_paint(self) -> Paint {
        Paint::Solid(self)
    }
}

impl IntoPaint for &WorkingColor {
    fn into_paint(self) -> Paint {
        Paint::Solid(*self)
    }
}

#[derive(Clone, Debug)]
enum Op {
    Fill {
        transform: Affine,
        shape: ShapeData,
        paint: Paint,
    },
    Stroke {
        transform: Affine,
        shape: ShapeData,
        stroke: Stroke,
        paint: Paint,
    },
    Shadow {
        transform: Affine,
        shape: ShapeData,
        shadow: Shadow,
    },
    Glyphs {
        transform: Affine,
        run: GlyphRun,
        paint: Paint,
    },
    Image {
        transform: Affine,
        image: ImageId,
        dst: Rect,
        sampling: Sampling,
    },
    Picture {
        transform: Affine,
        picture: Picture,
    },
    PushLayer {
        transform: Affine,
        clip: ShapeData,
        alpha: f32,
    },
    PopLayer,
}

/// One frame's drawing, recorded in window coordinates.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    ops: Vec<Op>,
}

impl Scene {
    /// An empty scene.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets everything recorded, keeping the allocation.
    pub fn reset(&mut self) {
        self.ops.clear();
    }

    /// Whether nothing has been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Whether the scene records anything that shows: a layer push alone does
    /// not.
    #[must_use]
    pub fn has_content(&self) -> bool {
        self.ops
            .iter()
            .any(|op| !matches!(op, Op::PushLayer { .. } | Op::PopLayer))
    }

    fn shape_data(style: Fill, shape: &(impl KurboShape + 'static)) -> ShapeData {
        match style {
            Fill::NonZero => ShapeData::of(shape),
            Fill::EvenOdd => ShapeData::of(&EvenOdd(shape.to_path(cherenkov::PATH_TOLERANCE))),
        }
    }

    /// Fills `shape`, positioned by `transform`, with `brush`. A brush
    /// transform maps the brush's own coordinates into the shape's.
    pub fn fill(
        &mut self,
        style: Fill,
        transform: Affine,
        brush: impl IntoPaint,
        brush_transform: Option<Affine>,
        shape: &(impl KurboShape + 'static),
    ) {
        let paint = transform_paint(brush.into_paint(), brush_transform);
        self.ops.push(Op::Fill {
            transform,
            shape: Self::shape_data(style, shape),
            paint,
        });
    }

    /// Strokes `shape`'s outline.
    pub fn stroke(
        &mut self,
        stroke: &Stroke,
        transform: Affine,
        brush: impl IntoPaint,
        brush_transform: Option<Affine>,
        shape: &(impl KurboShape + 'static),
    ) {
        let paint = transform_paint(brush.into_paint(), brush_transform);
        self.ops.push(Op::Stroke {
            transform,
            shape: ShapeData::of(shape),
            stroke: stroke.clone(),
            paint,
        });
    }

    /// Draws a blurred `shape`: a shadow with no offset.
    pub fn draw_blurred_shape(
        &mut self,
        transform: Affine,
        shape: &(impl KurboShape + 'static),
        color: WorkingColor,
        std_dev: f64,
    ) {
        self.ops.push(Op::Shadow {
            transform,
            shape: ShapeData::of(shape),
            shadow: Shadow {
                sigma: std_dev,
                offset: Vec2::ZERO,
                spread: 0.0,
                color,
            },
        });
    }

    /// Draws a blurred rounded rectangle.
    pub fn draw_blurred_rounded_rect(
        &mut self,
        transform: Affine,
        rect: Rect,
        color: WorkingColor,
        radius: f64,
        std_dev: f64,
    ) {
        self.draw_blurred_shape(transform, &rect.to_rounded_rect(radius), color, std_dev);
    }

    /// Draws a shadow of `shape`.
    pub fn draw_shadow(&mut self, transform: Affine, shape: &(impl KurboShape + 'static), shadow: Shadow) {
        self.ops.push(Op::Shadow {
            transform,
            shape: ShapeData::of(shape),
            shadow,
        });
    }

    /// Draws positioned glyphs of `font` at `size`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_glyphs(
        &mut self,
        transform: Affine,
        font: FontId,
        size: f32,
        coords: Vec<i16>,
        style: GlyphStyle,
        brush: impl IntoPaint,
        glyphs: impl IntoIterator<Item = Glyph>,
    ) {
        let glyphs: Vec<Glyph> = glyphs.into_iter().collect();
        if glyphs.is_empty() {
            return;
        }
        self.ops.push(Op::Glyphs {
            transform,
            run: GlyphRun {
                font,
                size,
                coords,
                glyphs,
                style,
            },
            paint: brush.into_paint(),
        });
    }

    /// Draws `image` into `dst`.
    pub fn draw_image(&mut self, transform: Affine, image: ImageId, dst: Rect, sampling: Sampling) {
        self.ops.push(Op::Image {
            transform,
            image,
            dst,
            sampling,
        });
    }

    /// Draws a recorded picture.
    pub fn draw_picture(&mut self, transform: Affine, picture: Picture) {
        self.ops.push(Op::Picture { transform, picture });
    }

    /// Appends another scene's drawing, positioned by `transform`.
    pub fn append(&mut self, other: &Self, transform: Option<Affine>) {
        let outer = transform.unwrap_or(Affine::IDENTITY);
        self.ops.extend(other.ops.iter().cloned().map(|op| match op {
            Op::Fill {
                transform,
                shape,
                paint,
            } => Op::Fill {
                transform: outer * transform,
                shape,
                paint,
            },
            Op::Stroke {
                transform,
                shape,
                stroke,
                paint,
            } => Op::Stroke {
                transform: outer * transform,
                shape,
                stroke,
                paint,
            },
            Op::Shadow {
                transform,
                shape,
                shadow,
            } => Op::Shadow {
                transform: outer * transform,
                shape,
                shadow,
            },
            Op::Glyphs {
                transform,
                run,
                paint,
            } => Op::Glyphs {
                transform: outer * transform,
                run,
                paint,
            },
            Op::Image {
                transform,
                image,
                dst,
                sampling,
            } => Op::Image {
                transform: outer * transform,
                image,
                dst,
                sampling,
            },
            Op::Picture { transform, picture } => Op::Picture {
                transform: outer * transform,
                picture,
            },
            Op::PushLayer {
                transform,
                clip,
                alpha,
            } => Op::PushLayer {
                transform: outer * transform,
                clip,
                alpha,
            },
            Op::PopLayer => Op::PopLayer,
        }));
    }

    /// Opens a layer clipped to `clip` and composited at `alpha`. Every push
    /// is balanced by a [`Self::pop_layer`] before the scene is lowered.
    pub fn push_layer(&mut self, alpha: f32, transform: Affine, clip: &(impl KurboShape + 'static)) {
        self.ops.push(Op::PushLayer {
            transform,
            clip: ShapeData::of(clip),
            alpha,
        });
    }

    /// Opens a layer clipped to an owned shape.
    pub fn push_layer_shape(&mut self, alpha: f32, transform: Affine, clip: ShapeData) {
        self.ops.push(Op::PushLayer {
            transform,
            clip,
            alpha,
        });
    }

    /// Layers pushed and not yet popped.
    #[must_use]
    pub fn open_layers(&self) -> usize {
        self.ops.iter().fold(0usize, |depth, op| match op {
            Op::PushLayer { .. } => depth + 1,
            Op::PopLayer => depth.saturating_sub(1),
            _ => depth,
        })
    }

    /// Opens a layer clipped to `clip` at full opacity.
    pub fn push_clip_layer(&mut self, transform: Affine, clip: &(impl KurboShape + 'static)) {
        self.push_layer(1.0, transform, clip);
    }

    /// Opens a layer clipped to `path`, filled with `style`.
    pub fn push_clip_path(&mut self, style: Fill, transform: Affine, path: &BezPath) {
        self.ops.push(Op::PushLayer {
            transform,
            clip: Self::shape_data(style, path),
            alpha: 1.0,
        });
    }

    /// Closes the innermost open layer.
    pub fn pop_layer(&mut self) {
        self.ops.push(Op::PopLayer);
    }

    /// Lowers the recording into a picture.
    ///
    /// # Panics
    /// Panics when a layer push has no matching pop.
    #[must_use]
    pub fn to_picture(&self) -> Picture {
        Picture::record(|recorder| {
            let mut index = 0;
            lower(&self.ops, &mut index, recorder);
            assert_eq!(
                index,
                self.ops.len(),
                "hydrolysis scene: layer pop without a matching push"
            );
        })
    }
}

fn lower(ops: &[Op], index: &mut usize, recorder: &mut StaticRecorder) {
    while *index < ops.len() {
        let op = &ops[*index];
        *index += 1;
        match op {
            Op::Fill {
                transform,
                shape,
                paint,
            } => recorder.transform(*transform, |r| r.fill(shape.clone(), paint.clone())),
            Op::Stroke {
                transform,
                shape,
                stroke,
                paint,
            } => recorder.transform(*transform, |r| {
                r.stroke(shape.clone(), stroke.clone(), paint.clone());
            }),
            Op::Shadow {
                transform,
                shape,
                shadow,
            } => recorder.transform(*transform, |r| r.shadow(shape.clone(), shadow.clone())),
            Op::Glyphs {
                transform,
                run,
                paint,
            } => recorder.transform(*transform, |r| r.glyphs(run, paint.clone())),
            Op::Image {
                transform,
                image,
                dst,
                sampling,
            } => recorder.transform(*transform, |r| r.image(*image, *dst, *sampling)),
            Op::Picture { transform, picture } => recorder.picture(picture, *transform),
            Op::PushLayer {
                transform,
                clip,
                alpha,
            } => {
                let alpha = *alpha;
                recorder.transform(*transform, |r| {
                    r.clip(clip.clone(), |r| {
                        if (alpha - 1.0).abs() < f32::EPSILON {
                            lower(ops, index, r);
                        } else {
                            let group = Group {
                                opacity: alpha,
                                ..Group::default()
                            };
                            r.group(group, |r| lower(ops, index, r));
                        }
                    });
                });
            }
            Op::PopLayer => return,
        }
    }
}

/// Maps a paint's own coordinates through `transform`. Solid colours and
/// image patterns carry their transform themselves; gradient geometry is
/// moved point by point, so a non-uniform scale on a radial gradient keeps
/// its centre and scales its radii by the transform's mean scale.
#[must_use]
pub fn transform_paint(paint: Paint, transform: Option<Affine>) -> Paint {
    let Some(transform) = transform else {
        return paint;
    };
    if transform == Affine::IDENTITY {
        return paint;
    }
    let scale = {
        let [a, b, c, d, _, _] = transform.as_coeffs();
        ((a * a + b * b).sqrt() + (c * c + d * d).sqrt()) / 2.0
    };
    match paint {
        Paint::Linear(mut gradient) => {
            gradient.start = transform * gradient.start;
            gradient.end = transform * gradient.end;
            Paint::Linear(gradient)
        }
        Paint::Radial(mut gradient) => {
            gradient.start_center = transform * gradient.start_center;
            gradient.end_center = transform * gradient.end_center;
            gradient.start_radius *= scale;
            gradient.end_radius *= scale;
            Paint::Radial(gradient)
        }
        Paint::Sweep(mut gradient) => {
            gradient.center = transform * gradient.center;
            Paint::Sweep(gradient)
        }
        Paint::Mesh(mesh) => Paint::Mesh(cherenkov::MeshGradient::new(
            mesh.columns(),
            mesh.rows(),
            mesh.points().iter().map(|p| transform * *p).collect(),
            mesh.colors().to_vec(),
        )),
        Paint::Image(mut pattern) => {
            pattern.transform = transform * pattern.transform;
            Paint::Image(pattern)
        }
        other @ (Paint::Solid(_) | Paint::Shader(_)) => other,
    }
}

/// A paint authored in unit space — `[0, 1]` on both axes — mapped onto
/// `bounds`.
#[must_use]
pub fn paint_in_bounds(paint: Paint, bounds: Rect) -> Paint {
    let map = Affine::translate((bounds.x0, bounds.y0))
        * Affine::scale_non_uniform(bounds.width(), bounds.height());
    transform_paint(paint, Some(map))
}

/// A working colour from gamma-encoded sRGB components and straight alpha.
#[must_use]
pub fn srgb([red, green, blue, alpha]: [f32; 4]) -> WorkingColor {
    use waterui_graphics::color::srgb_to_linear;
    waterui_graphics::color::working::from_linear_srgb(
        [srgb_to_linear(red), srgb_to_linear(green), srgb_to_linear(blue)],
        alpha,
    )
}

/// The point at `(x, y)`.
#[must_use]
pub fn point(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
