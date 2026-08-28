use anyhow::Result;
use std::sync::Arc;
use vello::peniko::Color;
use vello::RenderParams;
use wgpu::util::TextureBlitter;

/// GPU rendering context wrapping wgpu + vello.
///
/// Vello renders with a compute shader that writes to a storage texture, so it
/// cannot target a surface texture directly: surface textures are
/// `RENDER_ATTACHMENT`-only and typically sRGB. We therefore render into an
/// intermediate `Rgba8Unorm` + `STORAGE_BINDING` texture and blit that to the
/// surface. Drawing straight to the surface view fails at runtime with
/// "Storage texture binding expects format = Rgba8Unorm, but given a view with
/// format = Bgra8UnormSrgb".
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub vello_renderer: vello::Renderer,
    pub window: Arc<winit::window::Window>,
    /// Intermediate storage texture vello renders into.
    target_texture: wgpu::Texture,
    target_view: wgpu::TextureView,
    /// Copies `target_view` onto the surface texture.
    blitter: TextureBlitter,
}

impl GpuContext {
    /// Initialize GPU context from a winit window
    pub async fn new(window: Arc<winit::window::Window>) -> Result<Self> {
        let size = window.inner_size();

        // Create wgpu instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        // Create surface from window
        let surface = instance.create_surface(window.clone())?;

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| anyhow::anyhow!("No suitable GPU adapter found: {e}"))?;

        // Request device
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("uwebr Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await?;

        // Configure surface. A non-sRGB format is required: the blit shader
        // writes linear values already matching vello's output, so an sRGB
        // target would double-apply the transfer function and wash out colours.
        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
                )
            })
            .unwrap_or(surface_caps.formats[0]);

        let width = size.width.max(1);
        let height = size.height.max(1);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create vello renderer
        let vello_renderer = vello::Renderer::new(
            &device,
            vello::RendererOptions {
                use_cpu: false,
                ..Default::default()
            },
        )?;

        let (target_texture, target_view) = create_target(&device, width, height);
        let blitter = TextureBlitter::new(&device, format);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            vello_renderer,
            window,
            target_texture,
            target_view,
            blitter,
        })
    }

    /// Resize the surface and the intermediate render target
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.surface_config.width == width && self.surface_config.height == height {
            return;
        }

        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);

        // The storage texture must track the surface size or vello writes
        // outside the visible area.
        let (texture, view) = create_target(&self.device, width, height);
        self.target_texture = texture;
        self.target_view = view;
    }

    /// Render a vello scene: compute into the storage texture, then blit to screen.
    pub fn render_scene(&mut self, scene: &vello::Scene) -> Result<()> {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(tex)
            | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => tex,
            wgpu::CurrentSurfaceTexture::Timeout => return Ok(()),
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            other => return Err(anyhow::anyhow!("Surface error: {other:?}")),
        };

        let width = self.surface_config.width;
        let height = self.surface_config.height;

        self.vello_renderer.render_to_texture(
            &self.device,
            &self.queue,
            scene,
            &self.target_view,
            &RenderParams {
                base_color: Color::BLACK,
                width,
                height,
                antialiasing_method: vello::AaConfig::Area,
            },
        )?;

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("uwebr Surface Blit"),
            });
        self.blitter
            .copy(&self.device, &mut encoder, &self.target_view, &surface_view);
        self.queue.submit([encoder.finish()]);

        surface_texture.present();

        Ok(())
    }

    /// Get the window
    pub fn window(&self) -> &winit::window::Window {
        &self.window
    }

    /// Get surface dimensions
    pub fn size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    /// Format of the intermediate render target (always `Rgba8Unorm`).
    pub fn target_format(&self) -> wgpu::TextureFormat {
        self.target_texture.format()
    }
}

/// Create the intermediate texture vello's compute shader writes into.
fn create_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("uwebr Vello Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // STORAGE_BINDING for vello's compute write, TEXTURE_BINDING for the blit read.
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        format: wgpu::TextureFormat::Rgba8Unorm,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
