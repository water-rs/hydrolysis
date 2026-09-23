use std::collections::HashMap;

use vello::peniko::{Blob, ImageAlphaType, ImageData, ImageFormat};
use vello::{RenderParams, RendererOptions};
use waterui_graphics::{HybridImageAtlas, HybridRenderer, SceneEngine};

use crate::scene::{ClassicTarget, HybridTarget, WindowScene};

pub(crate) const HYBRID_TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub(crate) fn storage_usage_if_supported(
    device: &wgpu::Device,
    base: wgpu::TextureUsages,
) -> wgpu::TextureUsages {
    if device.limits().max_storage_textures_per_shader_stage > 0 {
        base | wgpu::TextureUsages::STORAGE_BINDING
    } else {
        base
    }
}

#[derive(Clone)]
pub(crate) struct RegisteredImage {
    pub(crate) image: ImageData,
    pub(crate) texture_id: vello_hybrid::TextureId,
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) generation: u64,
}

pub(crate) struct WindowSceneRenderer {
    rasterizer: WindowRasterizer,
    images: HashMap<u64, RegisteredImage>,
    next_generation: u64,
}

enum WindowRasterizer {
    Classic(Box<ClassicRasterizer>),
    Hybrid(Box<HybridRasterizer>),
}

struct ClassicRasterizer {
    renderer: vello::Renderer,
    scene: vello::Scene,
}

struct HybridRasterizer {
    renderer: HybridRenderer,
    scene: vello_hybrid::Scene,
}

impl WindowSceneRenderer {
    pub(crate) fn for_engine(
        engine: SceneEngine,
        device: &wgpu::Device,
        options: RendererOptions,
    ) -> Self {
        let rasterizer = match engine {
            SceneEngine::Classic => WindowRasterizer::Classic(Box::new(ClassicRasterizer {
                renderer: vello::Renderer::new(device, options)
                    .expect("the GPU device cannot rasterize vector scenes"),
                scene: vello::Scene::new(),
            })),
            SceneEngine::Hybrid => {
                let (renderer, resources) = vello_hybrid::Renderer::new(
                    device,
                    &vello_hybrid::RenderTargetConfig {
                        format: HYBRID_TARGET_FORMAT,
                        width: 1,
                        height: 1,
                    },
                );
                WindowRasterizer::Hybrid(Box::new(HybridRasterizer {
                    renderer: HybridRenderer {
                        renderer,
                        resources,
                        images: HybridImageAtlas::default(),
                    },
                    scene: vello_hybrid::Scene::new(1, 1),
                }))
            }
        };
        tracing::debug!(engine = ?engine, "window scene rasterizer selected");
        Self {
            rasterizer,
            images: HashMap::new(),
            next_generation: 0,
        }
    }

    pub(crate) fn engine(&self) -> SceneEngine {
        match &self.rasterizer {
            WindowRasterizer::Classic { .. } => SceneEngine::Classic,
            WindowRasterizer::Hybrid { .. } => SceneEngine::Hybrid,
        }
    }

    pub(crate) fn images_snapshot(&self) -> HashMap<u64, RegisteredImage> {
        self.images.clone()
    }

    pub(crate) fn sync_registered_images(&mut self, snapshot: &HashMap<u64, RegisteredImage>) {
        let stale: Vec<u64> = self
            .images
            .iter()
            .filter(|(id, registered)| {
                snapshot
                    .get(*id)
                    .is_none_or(|next| next.generation != registered.generation)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in stale {
            let registered = self
                .images
                .remove(&id)
                .expect("a registration collected as stale is still present");
            if let WindowRasterizer::Classic(rasterizer) = &mut self.rasterizer {
                let _ = rasterizer.renderer.override_image(&registered.image, None);
            }
        }
        for (id, registered) in snapshot {
            if self.images.contains_key(id) {
                continue;
            }
            if let WindowRasterizer::Classic(rasterizer) = &mut self.rasterizer {
                let _ = rasterizer.renderer.override_image(
                    &registered.image,
                    Some(wgpu::TexelCopyTextureInfoBase {
                        texture: registered.texture.clone(),
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    }),
                );
            }
            self.images.insert(*id, registered.clone());
        }
    }

    pub(crate) fn render_to_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &WindowScene,
        view: &wgpu::TextureView,
        params: &RenderParams,
    ) {
        match &mut self.rasterizer {
            WindowRasterizer::Classic(rasterizer) => {
                rasterizer.scene.reset();
                scene.replay(
                    &mut ClassicTarget::new(&mut rasterizer.scene),
                    vello::kurbo::Affine::IDENTITY,
                );
                rasterizer
                    .renderer
                    .render_to_texture(device, queue, &rasterizer.scene, view, params)
                    .expect("Vello scene render failed");
            }
            WindowRasterizer::Hybrid(rasterizer) => {
                let target = &mut rasterizer.scene;
                let renderer = &mut rasterizer.renderer;
                let width = u16::try_from(params.width)
                    .expect("hydrolysis render target exceeds hybrid scene width");
                let height = u16::try_from(params.height)
                    .expect("hydrolysis render target exceeds hybrid scene height");
                target.reset_and_resize(width, height);
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("hydrolysis hybrid scene"),
                });
                let mut bindings = vello_hybrid::TextureBindings::new();
                {
                    let mut target = HybridTarget::new(
                        target,
                        renderer,
                        device,
                        queue,
                        &mut encoder,
                        &self.images,
                        &mut bindings,
                    );
                    if params.base_color != vello::peniko::Color::TRANSPARENT {
                        target.fill_background(params.base_color, width, height);
                    }
                    scene.replay(&mut target, vello::kurbo::Affine::IDENTITY);
                }
                renderer
                    .renderer
                    .render(
                        target,
                        &mut renderer.resources,
                        device,
                        queue,
                        &mut encoder,
                        &vello_hybrid::RenderSize {
                            width: params.width,
                            height: params.height,
                        },
                        view,
                        &bindings,
                    )
                    .expect("hybrid scene render failed");
                queue.submit([encoder.finish()]);
            }
        }
    }

    pub(crate) fn register_texture(&mut self, texture: wgpu::Texture) -> ImageData {
        let image = ImageData {
            data: Blob::new(std::sync::Arc::new(Vec::<u8>::new())),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::AlphaPremultiplied,
            width: texture.width(),
            height: texture.height(),
        };
        let id = image.data.id();
        if let WindowRasterizer::Classic(rasterizer) = &mut self.rasterizer {
            let _ = rasterizer.renderer.override_image(
                &image,
                Some(wgpu::TexelCopyTextureInfoBase {
                    texture: texture.clone(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                }),
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let generation = self.next_generation;
        self.next_generation += 1;
        self.images.insert(
            id,
            RegisteredImage {
                image: image.clone(),
                texture_id: vello_hybrid::TextureId(id),
                texture,
                view,
                generation,
            },
        );
        image
    }

    pub(crate) fn override_image(
        &mut self,
        image: &ImageData,
        texture: Option<wgpu::TexelCopyTextureInfoBase<wgpu::Texture>>,
    ) {
        let Some(texture) = texture else {
            self.unregister_texture(image.clone());
            return;
        };
        assert_eq!(
            texture.mip_level, 0,
            "hydrolysis registered texture override: only mip level 0 is supported"
        );
        assert_eq!(
            texture.aspect,
            wgpu::TextureAspect::All,
            "hydrolysis registered texture override: only the full aspect is supported"
        );
        assert_eq!(
            texture.origin,
            wgpu::Origin3d::ZERO,
            "hydrolysis registered texture override: only origin (0,0,0) is supported"
        );
        let id = image.data.id();
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        if let WindowRasterizer::Classic(rasterizer) = &mut self.rasterizer {
            let _ = rasterizer
                .renderer
                .override_image(image, Some(texture.clone()));
        }
        let generation = self.next_generation;
        self.next_generation += 1;
        if let Some(registered) = self.images.get_mut(&id) {
            registered.texture = texture.texture;
            registered.view = view;
            registered.generation = generation;
        } else {
            self.images.insert(
                id,
                RegisteredImage {
                    image: image.clone(),
                    texture_id: vello_hybrid::TextureId(id),
                    texture: texture.texture,
                    view,
                    generation,
                },
            );
        }
    }

    pub(crate) fn unregister_texture(&mut self, image: ImageData) {
        if let WindowRasterizer::Classic(rasterizer) = &mut self.rasterizer {
            rasterizer.renderer.unregister_texture(image.clone());
        }
        self.images.remove(&image.data.id());
    }

    #[cfg(test)]
    pub(crate) fn is_registered(&self, image: &ImageData) -> bool {
        self.images.contains_key(&image.data.id())
    }
}
