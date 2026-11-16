//! GPU-Ausgabe mit wgpu: Das fertige Bild (CPU-gezeichnet mit tiny-skia) wird als Textur
//! hochgeladen und von der Grafikkarte aufs Fenster gebracht – inklusive Skalierung und
//! Vsync. Spart gegenüber softbuffer die Pixel-Umwandlung auf der CPU und das Warten auf den
//! Kompositor. Schlägt irgendetwas fehl, nimmt `Display` automatisch wieder softbuffer.

use std::sync::Arc;
use winit::window::Window;

const SHADER: &str = r#"
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VOut {
    // ein Dreieck, das den ganzen Bildschirm bedeckt
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    var o: VOut;
    o.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, y);
    return o;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;

@fragment
fn fs(v: VOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, smp, v.uv);
    return vec4<f32>(c.rgb, 1.0);
}
"#;

pub struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    linear: wgpu::Sampler,
    nearest: wgpu::Sampler,
    tex_format: wgpu::TextureFormat,
    tex: Option<(wgpu::Texture, wgpu::BindGroup, wgpu::BindGroup, (u32, u32))>,
    /// Beschreibung für Einstellungen/Log, z.B. „Vulkan · NVIDIA GeForce …“
    pub info: String,
}

/// Fehler bei der Ausgabe: `Fatal` = auf CPU zurückfallen
pub enum GpuErr { Skip, Fatal(String) }

impl Gpu {
    /// GPU-Ausgabe einrichten. `allow_software`: auch reine Software-Adapter (llvmpipe/WARP)
    /// zulassen – bei „Automatisch“ nicht, dort ist softbuffer schneller.
    pub fn new(window: Arc<Window>, vsync: bool, allow_software: bool) -> Result<Gpu, String> {
        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        let mut last_err = String::from("kein Grafikadapter");
        // erst die modernen Schnittstellen (Vulkan/Metal/DX12), dann OpenGL
        for backends in [wgpu::Backends::PRIMARY, wgpu::Backends::GL] {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends, ..Default::default() });
            let surface = match instance.create_surface(window.clone()) { Ok(s) => s, Err(e) => { last_err = format!("Surface: {e}"); continue; } };
            let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })) { Ok(a) => a, Err(e) => { last_err = format!("Adapter: {e}"); continue; } };
            let ai = adapter.get_info();
            if ai.device_type == wgpu::DeviceType::Cpu && !allow_software {
                last_err = format!("nur Software-Adapter ({})", ai.name);
                continue;
            }
            let (device, queue) = match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("jnd"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })) { Ok(d) => d, Err(e) => { last_err = format!("Gerät: {e}"); continue; } };
            let caps = surface.get_capabilities(&adapter);
            if caps.formats.is_empty() { last_err = "Surface ohne Formate".into(); continue; }
            // Ohne sRGB-Umrechnung: die Bytes kommen schon sRGB-kodiert aus tiny-skia
            let fmt = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
            let tex_format = if fmt.is_srgb() { wgpu::TextureFormat::Rgba8UnormSrgb } else { wgpu::TextureFormat::Rgba8Unorm };
            let present_mode = if vsync {
                if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) { wgpu::PresentMode::Mailbox } else { wgpu::PresentMode::Fifo }
            } else if caps.present_modes.contains(&wgpu::PresentMode::Immediate) { wgpu::PresentMode::Immediate } else { wgpu::PresentMode::AutoNoVsync };
            let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) { wgpu::CompositeAlphaMode::Opaque } else { caps.alpha_modes[0] };
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: fmt, width: w, height: h, present_mode, alpha_mode,
                view_formats: vec![], desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("present"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("present"),
                entries: &[
                    wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                    wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                ],
            });
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("present"), bind_group_layouts: &[&layout], push_constant_ranges: &[] });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("present"),
                layout: Some(&pl),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs"), targets: &[Some(wgpu::ColorTargetState { format: fmt, blend: None, write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });
            let mk = |f: wgpu::FilterMode| device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("present"), address_mode_u: wgpu::AddressMode::ClampToEdge, address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: f, min_filter: f, ..Default::default()
            });
            let (linear, nearest) = (mk(wgpu::FilterMode::Linear), mk(wgpu::FilterMode::Nearest));
            let info = format!("{:?} · {}", ai.backend, ai.name.trim());
            return Ok(Gpu { surface, device, queue, config, pipeline, layout, linear, nearest, tex_format, tex: None, info });
        }
        Err(last_err)
    }

    /// Bild zeigen: `rgba` = Pixel (vormultipliziert, deckend) in Größe sw×sh, skaliert aufs Fenster pw×ph
    pub fn present(&mut self, rgba: &[u8], sw: u32, sh: u32, pw: u32, ph: u32) -> Result<(), GpuErr> {
        if pw == 0 || ph == 0 || sw == 0 || sh == 0 { return Err(GpuErr::Skip); }
        if (self.config.width, self.config.height) != (pw, ph) {
            self.config.width = pw;
            self.config.height = ph;
            self.surface.configure(&self.device, &self.config);
        }
        if self.tex.as_ref().map(|t| t.3) != Some((sw, sh)) {
            let max = self.device.limits().max_texture_dimension_2d;
            if sw > max || sh > max { return Err(GpuErr::Fatal(format!("Bild {sw}x{sh} größer als Textur-Grenze {max}"))); }
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("frame"), size: wgpu::Extent3d { width: sw, height: sh, depth_or_array_layers: 1 },
                mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: self.tex_format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            let bg = |s: &wgpu::Sampler| self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("frame"), layout: &self.layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(s) }],
            });
            let (bl, bn) = (bg(&self.linear), bg(&self.nearest));
            self.tex = Some((tex, bl, bn, (sw, sh)));
        }
        let Some((tex, bl, bn, _)) = self.tex.as_ref() else { return Err(GpuErr::Skip); };
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(sw * 4), rows_per_image: Some(sh) },
            wgpu::Extent3d { width: sw, height: sh, depth_or_array_layers: 1 },
        );
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => { self.surface.configure(&self.device, &self.config); return Err(GpuErr::Skip); }
            Err(wgpu::SurfaceError::Timeout) => return Err(GpuErr::Skip),
            Err(e) => return Err(GpuErr::Fatal(format!("{e}"))),
        };
        let view = frame.texture.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("present") });
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
            });
            rp.set_pipeline(&self.pipeline);
            // gleiche Größe: Pixel 1:1 (scharf), sonst weich skalieren
            let same = sw <= pw && sh <= ph && pw - sw <= 3 && ph - sh <= 3;
            rp.set_bind_group(0, if same { bn } else { bl }, &[]);
            if same {
                // wie softbuffer: oben links ausrichten, 1–3 Pixel Rand bleiben schwarz
                rp.set_viewport(0.0, 0.0, sw as f32, sh as f32, 0.0, 1.0);
            }
            rp.draw(0..3, 0..1);
        }
        self.queue.submit(Some(enc.finish()));
        frame.present();
        Ok(())
    }
}
