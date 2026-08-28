use anyhow::Result;
use std::sync::Arc;
use vello::peniko::Color;
use vello::RenderParams;

/// GPU rendering context wrapping wgpu + vello
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub vello_renderer: vello::Renderer,
    pub window: Arc<winit::window::Window>,
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

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
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

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            vello_renderer,
            window,
        })
    }

    /// Resize the surface
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    /// Get the surface texture and render a vello scene to it
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

        let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.vello_renderer.render_to_texture(
            &self.device,
            &self.queue,
            scene,
            &view,
            &RenderParams {
                base_color: Color::BLACK,
                width: self.surface_config.width,
                height: self.surface_config.height,
                antialiasing_method: vello::AaConfig::Area,
            },
        )?;

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
}
