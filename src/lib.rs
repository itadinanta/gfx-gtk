extern crate epoxy;
extern crate gdk;
#[macro_use]
extern crate gfx;
extern crate gl;
extern crate gtk;
extern crate libc;
extern crate shared_library;

mod dl;
pub mod shaders;

use gfx::Factory;
use gfx_device_gl;
use std::ops::Fn;
use std::path::Path;

pub type Rgba = [f32; 4];
pub type Float4 = [f32; 4];
pub type Depth = f32;

pub mod formats {
	use gfx;

	pub type GtkTargetColorFormat = gfx::format::Rgba8;
	pub type GtkTargetColorView = <GtkTargetColorFormat as gfx::format::Formatted>::View;
	pub type DefaultRenderColorFormat = gfx::format::Rgba8;
	pub type DefaultRenderDepthFormat = gfx::format::DepthStencil;

	pub type RenderSurface<R, CF> = (
		gfx::handle::Texture<R, <CF as gfx::format::Formatted>::Surface>,
		gfx::handle::ShaderResourceView<R, <CF as gfx::format::Formatted>::View>,
		gfx::handle::RenderTargetView<R, CF>,
	);

	pub type DepthSurface<R, DF> = (
		gfx::handle::Texture<R, <DF as gfx::format::Formatted>::Surface>,
		gfx::handle::ShaderResourceView<R, <DF as gfx::format::Formatted>::View>,
		gfx::handle::DepthStencilView<R, DF>,
	);

	pub type RenderSurfaceWithDepth<R, CF, DF> = (
		gfx::handle::ShaderResourceView<R, <CF as gfx::format::Formatted>::View>,
		gfx::handle::RenderTargetView<R, CF>,
		gfx::handle::DepthStencilView<R, DF>,
	);

	pub const MSAA_NONE: gfx::texture::AaMode = gfx::texture::AaMode::Single;
	pub const MSAA_4X: gfx::texture::AaMode = gfx::texture::AaMode::Multi(4);
}

gfx_defines!(
	vertex BlitVertex {
		pos: [f32; 2] = "a_Pos",
		tex_coord: [f32; 2] = "a_TexCoord",
	}
	pipeline postprocess {
		vbuf: gfx::VertexBuffer<BlitVertex> = (),
		src: gfx::TextureSampler<formats::GtkTargetColorView> = "t_Source",
		dst: gfx::RenderTarget<formats::GtkTargetColorFormat> = "o_Color",
	}
);

#[allow(unused)]
pub struct GfxCallbackContext<D, F>
where
	D: gfx::Device,
	F: gfx::Factory<D::Resources>,
{
	pub device: D,
	pub factory: F,
	pub encoder: gfx::Encoder<D::Resources, D::CommandBuffer>,
}

impl<D, F> GfxCallbackContext<D, F>
where
	D: gfx::Device,
	F: gfx::Factory<D::Resources>,
{
	pub fn create_msaa_pipeline_state<I: gfx::pso::PipelineInit>(
		&mut self,
		aa: gfx::texture::AaMode,
		vertex_shader: &[u8],
		pixel_shader: &[u8],
		init: I,
	) -> std::result::Result<
		gfx::pso::PipelineState<D::Resources, I::Meta>,
		gfx::PipelineStateError<String>,
	> {
		self.factory
			.create_msaa_pipeline_state(aa, vertex_shader, pixel_shader, init)
	}
}

pub struct GfxPostprocessContext<D>
where
	D: gfx::Device,
{
	pub sampler: gfx::handle::Sampler<D::Resources>,
	pub pso: gfx::PipelineState<D::Resources, postprocess::Meta>,
	pub vbuf: gfx::handle::Buffer<D::Resources, BlitVertex>,
	pub ibuf: gfx::Slice<D::Resources>,
}

impl GfxPostprocessContext<GlDevice> {
	pub fn full_screen_blit<CF>(
		&self,
		encoder: &mut gfx::Encoder<GlResources, GlCommandBuffer>,
		render_screen: &GlFrameBufferTextureSrc<CF>,
		post_target: &GlFrameBuffer<formats::GtkTargetColorFormat>,
	) where
		CF: gfx::format::Formatted<View = formats::GtkTargetColorView>,
		CF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
		CF::Surface: gfx::format::RenderSurface + gfx::format::TextureSurface,
	{
		encoder.draw(
			&self.ibuf,
			&self.pso,
			&postprocess::Data {
				vbuf: self.vbuf.clone(),
				src: (render_screen.clone(), self.sampler.clone()),
				dst: (post_target.clone()),
			},
		);
	}
}

impl<D, F> GfxCallbackContext<D, F>
where
	D: gfx::Device,
	F: gfx::Factory<D::Resources>,
{
	pub fn flush(&mut self) {
		self.encoder.flush(&mut self.device);
	}
}

#[allow(unused)]
pub struct GfxContext<D, F, CF, DF>
where
	D: gfx::Device,
	F: gfx::Factory<D::Resources>,
	CF: gfx::format::Formatted,
{
	gfx_context: GfxCallbackContext<D, F>,
	viewport: Viewport,
	postprocess_context: GfxPostprocessContext<D>,
	postprocess_target: gfx::handle::RenderTargetView<D::Resources, formats::GtkTargetColorFormat>,
	render_target_source: gfx::handle::ShaderResourceView<D::Resources, CF::View>,
	render_target: gfx::handle::RenderTargetView<D::Resources, CF>,
	depth_buffer: gfx::handle::DepthStencilView<D::Resources, DF>,
}

pub type GlDevice = gfx_device_gl::Device;
pub type GlFactory = gfx_device_gl::Factory;
pub type GlCommandBuffer = gfx_device_gl::CommandBuffer;
pub type GlResources = <GlDevice as gfx::Device>::Resources;
pub type GlEncoder = gfx::Encoder<GlResources, GlCommandBuffer>;
pub type GlFrameBufferTextureSrc<F> =
	gfx::handle::ShaderResourceView<GlResources, <F as gfx::format::Formatted>::View>;
pub type GlFrameBuffer<CF> = gfx::handle::RenderTargetView<GlResources, CF>;
pub type GlDepthBuffer<DF> = gfx::handle::DepthStencilView<GlResources, DF>;
pub type GlGfxContext<CF, DF> = GfxContext<GlDevice, GlFactory, CF, DF>;

#[derive(Debug)]
pub enum Error {
	GenericError(String),
}

pub type Result<T> = std::result::Result<T, self::Error>;

impl<T: std::fmt::Display> std::convert::From<T> for self::Error {
	fn from(e: T) -> Self {
		self::Error::GenericError(e.to_string())
	}
}

pub trait FactoryExt<R: gfx::Resources>: gfx::traits::FactoryExt<R> {
	fn create_gtk_compatible_targets<CF, DF>(
		&mut self,
		aa: gfx::texture::AaMode,
		width: gfx::texture::Size,
		height: gfx::texture::Size,
	) -> Result<formats::RenderSurfaceWithDepth<R, CF, DF>>
	where
		CF: gfx::format::Formatted,
		CF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
		CF::Surface: gfx::format::RenderSurface + gfx::format::TextureSurface,
		DF: gfx::format::Formatted,
		DF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
		DF::Surface: gfx::format::DepthSurface + gfx::format::TextureSurface,
	{
		let (_, color_resource, color_target) =
			self.create_gtk_compatible_render_target(aa, width, height)?;
		let (_, _, depth_target) = self.create_gtk_compatible_depth_target(aa, width, height)?;
		Ok((color_resource, color_target, depth_target))
	}

	fn create_msaa_pipeline_state<I: gfx::pso::PipelineInit>(
		&mut self,
		aa: gfx::texture::AaMode,
		vertex_shader: &[u8],
		pixel_shader: &[u8],
		init: I,
	) -> std::result::Result<gfx::pso::PipelineState<R, I::Meta>, gfx::PipelineStateError<String>>
	{
		let shaders = self.create_shader_set(vertex_shader, pixel_shader)?;

		let rasterizer = match aa {
			gfx::texture::AaMode::Multi(_) => gfx::state::Rasterizer {
				samples: Some(gfx::state::MultiSample),
				..gfx::state::Rasterizer::new_fill()
			},
			_ => gfx::state::Rasterizer::new_fill(),
		};

		self.create_pipeline_state(&shaders, gfx::Primitive::TriangleList, rasterizer, init)
	}

	fn create_gtk_compatible_depth_target<D>(
		&mut self,
		aa: gfx::texture::AaMode,
		width: gfx::texture::Size,
		height: gfx::texture::Size,
	) -> Result<formats::DepthSurface<R, D>>
	where
		D: gfx::format::Formatted,
		D::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
		D::Surface: gfx::format::DepthSurface + gfx::format::TextureSurface,
	{
		let kind = gfx::texture::Kind::D2(width, height, aa);
		let tex = self.create_texture(
			kind,
			1,
			gfx::memory::Bind::SHADER_RESOURCE | gfx::memory::Bind::DEPTH_STENCIL,
			gfx::memory::Usage::Data,
			Some(<D as gfx::format::Formatted>::get_format().1),
		)?;
		let resource =
			self.view_texture_as_shader_resource::<D>(&tex, (0, 0), gfx::format::Swizzle::new())?;
		let target = self.view_texture_as_depth_stencil_trivial(&tex)?;
		Ok((tex, resource, target))
	}

	fn create_gtk_compatible_render_target<F>(
		&mut self,
		aa: gfx::texture::AaMode,
		width: gfx::texture::Size,
		height: gfx::texture::Size,
	) -> Result<formats::RenderSurface<R, F>>
	where
		F: gfx::format::Formatted,
		F::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
		F::Surface: gfx::format::RenderSurface + gfx::format::TextureSurface,
	{
		let kind = gfx::texture::Kind::D2(width, height, aa);
		let tex = self.create_texture(
			kind,
			1,
			gfx::memory::Bind::SHADER_RESOURCE | gfx::memory::Bind::RENDER_TARGET,
			gfx::memory::Usage::Data,
			Some(<F as gfx::format::Formatted>::get_format().1),
		)?;
		let hdr_srv =
			self.view_texture_as_shader_resource::<F>(&tex, (0, 0), gfx::format::Swizzle::new())?;
		let hdr_color_buffer = self.view_texture_as_render_target(&tex, 0, None)?;
		Ok((tex, hdr_srv, hdr_color_buffer))
	}
}

impl<F, R> FactoryExt<R> for F
where
	F: gfx::Factory<R>,
	R: gfx::Resources,
{
}

pub fn load() {
	use self::dl::{fn_from, DlProcLoader, Failover};
	let loader = Failover(
		DlProcLoader::current_module(),
		Failover(
			DlProcLoader::open(Path::new("libepoxy-0")),
			Failover(
				DlProcLoader::open(Path::new("libepoxy0")),
				DlProcLoader::open(Path::new("libepoxy")),
			),
		),
	);
	epoxy::load_with(fn_from(loader));
	gl::load_with(epoxy::get_proc_addr);
}

pub fn debug_load() {
	use self::dl::{debug_get_proc_addr, fn_from, DlProcLoader, Failover};
	let loader = Failover(
		DlProcLoader::current_module(),
		Failover(
			DlProcLoader::open(Path::new("libepoxy-0")),
			Failover(
				DlProcLoader::open(Path::new("libepoxy0")),
				DlProcLoader::open(Path::new("libepoxy")),
			),
		),
	);
	epoxy::load_with(fn_from(loader));
	gl::load_with(debug_get_proc_addr);
}

#[derive(Clone, Copy, Debug)]
pub enum GlRenderCallbackStatus {
	Complete,
	NoFlush,
}

pub type GlCallbackContext = GfxCallbackContext<GlDevice, GlFactory>;
pub type GlPostprocessContext = GfxPostprocessContext<GlDevice>;

#[derive(Clone)]
pub struct Viewport {
	pub width: i32,
	pub height: i32,
	pub target_width: i32,
	pub target_height: i32,
	pub aa: gfx::texture::AaMode,
}

impl Viewport {
	pub fn aspect_ratio(&self) -> f32 {
		self.width as f32 / self.height as f32
	}

	pub fn with_aa(aa: gfx::texture::AaMode, target_width: i32, target_height: i32) -> Self {
		// for supersampling
		let (width, height) = Self::aa_size(aa, target_width, target_height);
		Viewport {
			width,
			height,
			target_width,
			target_height,
			aa,
		}
	}

	fn aa_size(aa: gfx::texture::AaMode, width: i32, height: i32) -> (i32, i32) {
		let (mx, my) = match aa {
			gfx::texture::AaMode::Single => (1, 1),
			gfx::texture::AaMode::Multi(_) => (1, 1),
			_ => (0, 0),
		};
		(width * mx, height * my)
	}
}

pub trait GlRenderCallback<CF, DF>
where
	CF: gfx::format::Formatted<View = formats::GtkTargetColorView>,
	CF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
	CF::Surface: gfx::format::RenderSurface + gfx::format::TextureSurface,
	DF: gfx::format::Formatted,
	DF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
	DF::Surface: gfx::format::DepthSurface + gfx::format::TextureSurface,
{
	fn render(
		&mut self,
		gfx_context: &mut GlCallbackContext,
		viewport: &Viewport,
		render_target: &GlFrameBuffer<CF>,
		depth_buffer: &GlDepthBuffer<DF>,
	) -> Result<GlRenderCallbackStatus>;

	fn resize(
		&mut self,
		_gfx_context: &mut GlCallbackContext,
		_viewport: Viewport,
	) -> Result<GlRenderCallbackStatus> {
		Ok(GlRenderCallbackStatus::Complete)
	}
}

pub trait GlPostprocessCallback<CF, DF>
where
	CF: gfx::format::Formatted<View = formats::GtkTargetColorView>,
	CF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
	CF::Surface: gfx::format::RenderSurface + gfx::format::TextureSurface,
	DF: gfx::format::Formatted,
	DF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
	DF::Surface: gfx::format::DepthSurface + gfx::format::TextureSurface,
{
	fn postprocess(
		&mut self,
		gfx_context: &mut GlCallbackContext,
		postprocess_context: &GlPostprocessContext,
		_viewport: &Viewport,
		render_screen: &GlFrameBufferTextureSrc<CF>,
		post_target: &GlFrameBuffer<formats::GtkTargetColorFormat>,
	) -> Result<GlRenderCallbackStatus> {
		postprocess_context.full_screen_blit::<CF>(
			&mut gfx_context.encoder,
			render_screen,
			post_target,
		);
		gfx_context.flush();
		Ok(GlRenderCallbackStatus::Complete)
	}
}

impl<CF, DF> GlGfxContext<CF, DF>
where
	CF: gfx::format::Formatted<View = [f32; 4]>,
	CF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
	CF::Surface: gfx::format::RenderSurface + gfx::format::TextureSurface,
	DF: gfx::format::Formatted,
	DF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
	DF::Surface: gfx::format::DepthSurface + gfx::format::TextureSurface,
{
	pub fn new(
		aa: gfx::texture::AaMode,
		widget_width: i32,
		widget_height: i32,
	) -> Result<GlGfxContext<CF, DF>> {
		Self::new_with_loader(aa, widget_width, widget_height, &epoxy::get_proc_addr)
	}

	pub fn new_with_loader(
		aa: gfx::texture::AaMode,
		widget_width: i32,
		widget_height: i32,
		get_proc_addr: &Fn(&str) -> *const std::ffi::c_void,
	) -> Result<GlGfxContext<CF, DF>> {
		use self::FactoryExt as LocalFactory;
		use gfx::traits::FactoryExt;

		let (device, mut factory) = gfx_device_gl::create(get_proc_addr);
		let encoder = factory.create_command_buffer().into();
		let viewport = Viewport::with_aa(aa, widget_width, widget_height);

		let (render_target_source, render_target, depth_buffer) = factory
			.create_gtk_compatible_targets(aa, viewport.width as u16, viewport.height as u16)?;

		let (_, _, postprocess_target) = factory.create_gtk_compatible_render_target(
			formats::MSAA_NONE,
			viewport.target_width as u16,
			viewport.target_height as u16,
		)?;

		let full_screen_triangle = vec![
			BlitVertex {
				pos: [-1., -1.],
				tex_coord: [0., 0.],
			},
			BlitVertex {
				pos: [-1., 3.],
				tex_coord: [0., 2.],
			},
			BlitVertex {
				pos: [3., -1.],
				tex_coord: [2., 0.],
			},
		];

		let full_screen_triangle_index = vec![0u16, 2, 1];

		let (vbuf, ibuf) = factory.create_vertex_buffer_with_slice(
			&full_screen_triangle,
			&full_screen_triangle_index[..],
		);

		let nearest_sampler = factory.create_sampler(gfx::texture::SamplerInfo::new(
			gfx::texture::FilterMethod::Scale,
			gfx::texture::WrapMode::Clamp,
		));

		// TODO: make this configurable
		let pixel_shader_code = match viewport.aa {
			gfx::texture::AaMode::Multi(4) => shaders::POST_PIXEL_SHADER_MSAA_4X.as_bytes(),
			_ => shaders::POST_PIXEL_SHADER.as_bytes(),
		};

		let post_pso = factory.create_pipeline_simple(
			shaders::POST_VERTEX_SHADER.as_bytes(),
			pixel_shader_code,
			postprocess::new(),
		)?;

		let postprocess_context = GfxPostprocessContext {
			vbuf,
			ibuf,
			pso: post_pso,
			sampler: nearest_sampler,
		};

		let gfx_context = GlCallbackContext {
			device,
			factory,
			encoder,
		};

		Ok(GfxContext {
			gfx_context,
			viewport,
			postprocess_context,
			render_target_source,
			render_target,
			depth_buffer,
			postprocess_target,
		})
	}

	pub fn gfx_context_mut(&mut self) -> &mut GlCallbackContext {
		&mut self.gfx_context
	}

	pub fn viewport(&self) -> &Viewport {
		&self.viewport
	}

	pub fn resize(&mut self, widget_width: i32, widget_height: i32) -> Result<()> {
		let new_viewport = Viewport::with_aa(self.viewport.aa, widget_width, widget_height);
		if new_viewport.width != self.viewport.width || new_viewport.height != self.viewport.height
		{
			let (frame_buffer_source, frame_buffer, depth_buffer) =
				self.gfx_context.factory.create_gtk_compatible_targets(
					self.viewport.aa,
					new_viewport.width as u16,
					new_viewport.height as u16,
				)?;

			let (_, _, postprocess_target) = self
				.gfx_context
				.factory
				.create_gtk_compatible_render_target(
					formats::MSAA_NONE,
					new_viewport.target_width as u16,
					new_viewport.target_height as u16,
				)?;

			self.viewport = new_viewport;
			self.render_target_source = frame_buffer_source;
			self.render_target = frame_buffer;
			self.postprocess_target = postprocess_target;
			self.depth_buffer = depth_buffer;
		}

		Ok(())
	}

	pub fn with_gfx<R>(&mut self, render_callback: &mut R)
	where
		R: GlRenderCallback<CF, DF> + GlPostprocessCallback<CF, DF>,
	{
		fn get_current_draw_framebuffer_name() -> u32 {
			let mut framebuffer_name = 0;
			unsafe {
				gl::GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut framebuffer_name);
			}
			framebuffer_name as u32
		}

		fn get_current_renderbuffer_binding() -> u32 {
			let mut renderbuffer_binding = 0;
			unsafe {
				gl::GetIntegerv(gl::RENDERBUFFER_BINDING, &mut renderbuffer_binding);
			}
			renderbuffer_binding as u32
		}

		// we need to keep track of the framebuffer Gtk wants to render to,
		// which has been bound in the current gl_context, by the GlArea machinery
		let gtk_framebuffer_name = get_current_draw_framebuffer_name();
		let gtk_renderbuffer_binding = get_current_renderbuffer_binding();
		// we do some GFX rendering, will knacker the buffer bindings but end up with a surface
		// we can blit from
		GlRenderCallback::render(
			render_callback,
			&mut self.gfx_context,
			&self.viewport,
			&self.render_target,
			&self.depth_buffer,
		)
		.ok(); // TODO: handle error

		GlPostprocessCallback::postprocess(
			render_callback,
			&mut self.gfx_context,
			&self.postprocess_context,
			&self.viewport,
			&self.render_target_source,
			&self.postprocess_target,
		)
		.ok(); // TODO: handle error

		// we have a full frame here and GFX shouldn't have thrown away the current
		// framebuffer bindings, yet, so we can grab it
		let gfx_framebuffer_name = get_current_draw_framebuffer_name();
		unsafe {
			// we want the framebuffer from Gfx (which we have just got) as the blit source
			gl::BindFramebuffer(gl::READ_FRAMEBUFFER, gfx_framebuffer_name);
			// and the framebuffer from Gtk (which we have saved earlier) as the destination
			gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, gtk_framebuffer_name);
			// we need to re-attach the color attachment buffer as well
			gl::NamedFramebufferRenderbuffer(
				gtk_framebuffer_name,
				gl::COLOR_ATTACHMENT0,
				gl::RENDERBUFFER,
				gtk_renderbuffer_binding,
			);
			// And finally, we blit the GFX framebuffer onto the GlArea framebuffer.
			// This is wasteful as the GlArea code already does this for its own off-screen
			// framebuffer target but we have no means to blit directly to the screen backbuffer
			// as it happens under the hood within the GlArea rendering code
			gl::BlitFramebuffer(
				0,
				0,
				self.viewport.target_width,
				self.viewport.target_height,
				0,
				0,
				self.viewport.target_width,
				self.viewport.target_height,
				gl::COLOR_BUFFER_BIT,
				gl::NEAREST,
			);
			gl::Flush();
		}
		self.cleanup();
	}

	fn cleanup(&mut self) {
		use gfx::Device;
		self.gfx_context.device.cleanup();
	}
}
