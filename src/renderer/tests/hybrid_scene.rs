use super::test_environment;
use super::tree::text_node;
use crate::platform::{OffscreenWindow, SurfaceProvider};
use crate::renderer::HydrolysisRenderer;
use crate::renderer::RenderContext;
use crate::scene::WindowScene;
use crate::scene_renderer::WindowSceneRenderer;
use vello::kurbo::{Affine, Rect};
use vello::peniko::{BlendMode, Brush, Color, Fill, ImageBrush};
use waterui_core::layout::Size;
use waterui_graphics::SceneEngine;
use waterui_testing::TestArtifacts;

const WIDTH: u32 = 96;
const HEIGHT: u32 = 96;

fn export_path(case: &str, stage: &str) -> std::path::PathBuf {
    let path = TestArtifacts::new("hydrolysis").snapshot_path(case, stage);
    std::fs::create_dir_all(path.parent().expect("a snapshot path has a case directory"))
        .expect("the export directory must be creatable");
    path
}

fn target_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hydrolysis-hybrid-test-target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

fn source_texture(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [u8; 4]) -> wgpu::Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hydrolysis-hybrid-test-source"),
        size: wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let pixels = vec![rgba; 64].into_flattened();
    queue.write_texture(
        texture.as_image_copy(),
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(32),
            rows_per_image: Some(8),
        },
        wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
    );
    texture
}

fn render_to_png(
    renderer: &mut WindowSceneRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    scene: &WindowScene,
    target: &wgpu::Texture,
    view: &wgpu::TextureView,
    stage: &str,
) -> std::path::PathBuf {
    renderer.render_to_texture(
        device,
        queue,
        scene,
        view,
        &vello::RenderParams {
            base_color: Color::WHITE,
            width: target.width(),
            height: target.height(),
            antialiasing_method: vello::AaConfig::Area,
        },
    );
    let rgba8 = crate::readback::readback_texture_rgba8(
        device,
        queue,
        target,
        target.width(),
        target.height(),
    );
    let path = export_path("hybrid_scene", stage);
    image::RgbaImage::from_raw(target.width(), target.height(), rgba8)
        .expect("readback dimensions must match the rgba buffer")
        .save(&path)
        .expect("snapshot png must be writable");
    path
}

#[test]
fn hybrid_scene_renderer_draws_the_window_scene_contract() {
    let env = test_environment();
    let platform = OffscreenWindow::new_for_tests(WIDTH, HEIGHT, wgpu::TextureFormat::Rgba8Unorm);
    let device;
    let queue;
    let adapter;
    {
        let surface = platform.surface_ref();
        adapter = surface.adapter().clone();
        device = surface.device().clone();
        queue = surface.queue().clone();
    }
    let mut renderer = HydrolysisRenderer::new(&adapter, &device);
    renderer.set_frame_resources(&adapter, &device, &queue);

    let mut node = text_node("hybrid scene");
    let bounds = Rect::new(0.0, 0.0, f64::from(WIDTH), f64::from(HEIGHT));
    node.layout(&mut renderer, &env, Size::new(96.0, 96.0));
    renderer.reset_scene();
    renderer.begin_rebuild_frame();
    node.flush(
        &mut renderer,
        RenderContext::with_transforms(bounds, Affine::IDENTITY, Affine::IDENTITY),
        &env,
    );
    let text_scene = core::mem::take(renderer.scene_mut());
    renderer.finish_rebuild_frame();
    assert!(text_scene.has_content());

    let mut hybrid = WindowSceneRenderer::for_engine(
        SceneEngine::Hybrid,
        &device,
        vello::RendererOptions::default(),
    );
    assert_eq!(hybrid.engine(), SceneEngine::Hybrid);

    let registered_texture = source_texture(&device, &queue, [100, 16, 16, 128]);
    let registered = hybrid.register_texture(registered_texture);
    assert!(hybrid.is_registered(&registered));

    let build_scene = |registered: Option<&vello::peniko::ImageData>| {
        let mut scene = WindowScene::new();
        scene.append(&text_scene, Some(Affine::translate((4.0, 4.0))));
        let clip = Rect::new(8.0, 24.0, 88.0, 64.0);
        scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &clip);
        scene.fill(
            Fill::NonZero,
            Affine::translate((12.0, 28.0)) * Affine::rotate(0.4),
            &Brush::Solid(Color::new([0.1, 0.35, 0.9, 1.0])),
            None,
            &Rect::new(0.0, 0.0, 40.0, 24.0),
        );
        scene.push_layer(
            Fill::NonZero,
            BlendMode::default(),
            0.6,
            Affine::IDENTITY,
            &Rect::new(8.0, 24.0, 88.0, 64.0),
        );
        if let Some(image) = registered {
            scene.draw_image(
                &ImageBrush::new(image.clone()),
                Affine::translate((48.0, 32.0)) * Affine::scale(4.0),
            );
        }
        scene.pop_layer();
        scene.pop_layer();
        scene.draw_blurred_rounded_rect(
            Affine::translate((16.0, 68.0)),
            Rect::new(0.0, 0.0, 64.0, 16.0),
            Color::new([0.15, 0.6, 0.25, 1.0]),
            8.0,
            3.0,
        );
        assert!(scene.has_content());
        assert_eq!(scene.open_layer_count(), 0);
        scene
    };

    let target = target_texture(&device, WIDTH, HEIGHT);
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let first = render_to_png(
        &mut hybrid,
        &device,
        &queue,
        &build_scene(Some(&registered)),
        &target,
        &view,
        "frame_one",
    );

    let replacement = source_texture(&device, &queue, [16, 80, 110, 128]);
    hybrid.override_image(
        &registered,
        Some(wgpu::TexelCopyTextureInfoBase {
            texture: replacement,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        }),
    );
    assert!(hybrid.is_registered(&registered));
    let second = render_to_png(
        &mut hybrid,
        &device,
        &queue,
        &build_scene(Some(&registered)),
        &target,
        &view,
        "frame_two_override",
    );

    hybrid.unregister_texture(registered.clone());
    assert!(!hybrid.is_registered(&registered));
    let unregistered = render_to_png(
        &mut hybrid,
        &device,
        &queue,
        &build_scene(None),
        &target,
        &view,
        "unregistered",
    );

    let resized = target_texture(&device, 128, 64);
    let resized_view = resized.create_view(&wgpu::TextureViewDescriptor::default());
    let resized_png = render_to_png(
        &mut hybrid,
        &device,
        &queue,
        &build_scene(None),
        &resized,
        &resized_view,
        "resized",
    );

    tracing::info!(
        first = %first.display(),
        second = %second.display(),
        unregistered = %unregistered.display(),
        resized = %resized_png.display(),
        "hybrid scene renderer screenshots"
    );
}
