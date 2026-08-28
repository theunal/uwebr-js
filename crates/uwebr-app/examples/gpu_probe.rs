//! Headless verification that the render pipeline puts pixels on screen.
//!
//! Runs the full Element → layout → scene → vello → GPU path without opening a
//! window, reads the framebuffer back, and reports what it found. This is the
//! automatable half of "does the app actually render?"; the remaining half
//! (a real swapchain) still needs `cargo run` on a desktop.
//!
//! Run with `cargo run -p uwebr-app --example gpu_probe`.

use uwebr_app::RenderPipeline;
use uwebr_core::component::{Element, NodeType, PropValue};
use vello::peniko::Color;
use vello::RenderParams;

const WIDTH: u32 = 400;
const HEIGHT: u32 = 200;

const CSS: &str = r#"
.app {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100vh;
    background-color: #1a1a2e;
    color: #e0e0e0;
}
h1 { font-size: 2rem; }
"#;

fn scaffold() -> Element {
    Element {
        node_type: NodeType::Element("div".into()),
        props: vec![("class".into(), PropValue::String("app".into()))],
        children: vec![Element {
            node_type: NodeType::Element("h1".into()),
            props: vec![],
            children: vec![Element {
                node_type: NodeType::Text("Hello from uwebr!".into()),
                props: vec![],
                children: vec![],
            }],
        }],
    }
}

fn main() -> anyhow::Result<()> {
    let mut pipeline = RenderPipeline::new().with_css(CSS);
    pipeline.build_render_scene(&scaffold(), WIDTH, HEIGHT);

    println!("render nodes: {}", pipeline.render_scene().node_count());
    for node in pipeline.render_scene().nodes() {
        println!("  {:?} at {:?}", node.kind, node.layout);
    }

    let scene = pipeline.render(&scaffold(), WIDTH, HEIGHT);
    let enc = scene.encoding();
    println!(
        "encoded: {} glyphs, {} paths",
        enc.resources.glyphs.len(),
        enc.n_paths
    );

    let pixels = pollster::block_on(render_offscreen(&scene))?;
    report(&pixels);
    Ok(())
}

/// Render a scene into an `Rgba8Unorm` storage texture and read it back.
async fn render_offscreen(scene: &vello::Scene) -> anyhow::Result<Vec<u8>> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .map_err(|e| anyhow::anyhow!("no adapter: {e}"))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("gpu_probe"),
            ..Default::default()
        })
        .await?;

    let mut renderer = vello::Renderer::new(
        &device,
        vello::RendererOptions {
            use_cpu: false,
            ..Default::default()
        },
    )?;

    // Same layout as GpuContext: storage for the compute write, copy for readback.
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        format: wgpu::TextureFormat::Rgba8Unorm,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    renderer.render_to_texture(
        &device,
        &queue,
        scene,
        &view,
        &RenderParams {
            base_color: Color::BLACK,
            width: WIDTH,
            height: HEIGHT,
            antialiasing_method: vello::AaConfig::Area,
        },
    )?;

    // Buffer rows must be 256-byte aligned for texture-to-buffer copies.
    let unpadded = WIDTH * 4;
    let padded = unpadded.div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (padded * HEIGHT) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    })?;
    rx.recv()??;

    let data = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((unpadded * HEIGHT) as usize);
    for row in 0..HEIGHT {
        let start = (row * padded) as usize;
        pixels.extend_from_slice(&data[start..start + unpadded as usize]);
    }
    drop(data);
    buffer.unmap();

    Ok(pixels)
}

/// Summarise the framebuffer: distinct colours and whether text was drawn.
fn report(pixels: &[u8]) {
    use std::collections::HashMap;

    let mut counts: HashMap<[u8; 3], usize> = HashMap::new();
    for px in pixels.as_chunks::<4>().0 {
        let [r, g, b, _a] = *px;
        *counts.entry([r, g, b]).or_default() += 1;
    }

    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by_key(|(_, n)| std::cmp::Reverse(*n));

    println!("distinct colours: {}", sorted.len());
    for ([r, g, b], n) in sorted.iter().take(5) {
        println!("  #{r:02x}{g:02x}{b:02x}  {n} px");
    }

    let background = sorted.first().map(|(c, _)| *c);
    let non_background = sorted.iter().skip(1).map(|(_, n)| *n).sum::<usize>();

    match background {
        Some([0x1a, 0x1a, 0x2e]) => println!("OK: .app background #1a1a2e covers the surface"),
        Some([r, g, b]) => {
            println!("WARN: dominant colour is #{r:02x}{g:02x}{b:02x}, expected #1a1a2e")
        }
        None => println!("WARN: no pixels"),
    }

    if non_background > 0 {
        println!("OK: {non_background} px of foreground (text glyphs)");
    } else {
        println!("WARN: nothing drawn on top of the background");
    }
}
