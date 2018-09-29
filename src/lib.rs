extern crate epoxy;
extern crate gdk;
extern crate gfx;
extern crate gl;
extern crate gtk;
extern crate libc;
extern crate shared_library;

use gfx_device_gl;
use std::ptr;

pub type Rgba = [f32; 4];
pub type Float4 = [f32; 4];
pub type Depth = f32;

pub mod formats {
	use gfx;

	pub type RenderColorFormat = gfx::format::Srgba8;
	pub type RenderDepthFormat = gfx::format::DepthStencil;

	pub type RenderSurface<R> = (
		gfx::handle::Texture<R, <RenderColorFormat as gfx::format::Formatted>::Surface>,
		gfx::handle::ShaderResourceView<R, <RenderColorFormat as gfx::format::Formatted>::View>,
		gfx::handle::RenderTargetView<R, RenderColorFormat>,
	);

	pub type DepthSurface<R> = (
		gfx::handle::Texture<R, <RenderDepthFormat as gfx::format::Formatted>::Surface>,
		gfx::handle::ShaderResourceView<R, <RenderDepthFormat as gfx::format::Formatted>::View>,
		gfx::handle::DepthStencilView<R, RenderDepthFormat>,
	);

	pub type RenderSurfaceWithDepth<R> = (
		gfx::handle::ShaderResourceView<R, <RenderColorFormat as gfx::format::Formatted>::View>,
		gfx::handle::RenderTargetView<R, RenderColorFormat>,
		gfx::handle::DepthStencilView<R, RenderDepthFormat>,
	);
	pub const MSAA_MODE: gfx::texture::AaMode = gfx::texture::AaMode::Single;
}

#[allow(unused)]
pub struct GfxContext<D, F>
where
	D: gfx::Device,
	F: gfx::Factory<D::Resources>,
{
	device: D,
	factory: F,
	encoder: gfx::Encoder<D::Resources, D::CommandBuffer>,
	width: gfx::texture::Size,
	height: gfx::texture::Size,
	frame_buffer_source: gfx::handle::ShaderResourceView<D::Resources, Rgba>,
	frame_buffer: gfx::handle::RenderTargetView<D::Resources, formats::RenderColorFormat>,
	depth_buffer: gfx::handle::DepthStencilView<D::Resources, formats::RenderDepthFormat>,
}

pub type GlDevice = gfx_device_gl::Device;
pub type GlFactory = gfx_device_gl::Factory;
pub type GlCommandBuffer = gfx_device_gl::CommandBuffer;
pub type GlResources = <GlDevice as gfx::Device>::Resources;
pub type GlEncoder = gfx::Encoder<GlResources, GlCommandBuffer>;
pub type GlFrameBuffer = gfx::handle::RenderTargetView<GlResources, formats::RenderColorFormat>;
pub type GlDepthBuffer = gfx::handle::DepthStencilView<GlResources, formats::RenderDepthFormat>;
pub type GlGfxContext = GfxContext<GlDevice, GlFactory>;

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
	fn create_gtk_compatible_targets(
		&mut self,
		width: gfx::texture::Size,
		height: gfx::texture::Size,
	) -> Result<formats::RenderSurfaceWithDepth<R>> {
		let (_, color_resource, color_target) =
			self.create_gtk_compatible_render_target(formats::MSAA_MODE, width, height)?;
		let (_, _, depth_target) =
			self.create_gtk_compatible_depth_target(formats::MSAA_MODE, width, height)?;
		Ok((color_resource, color_target, depth_target))
	}

	fn create_gtk_compatible_depth_target(
		&mut self,
		aa: gfx::texture::AaMode,
		width: gfx::texture::Size,
		height: gfx::texture::Size,
	) -> Result<formats::DepthSurface<R>> {
		let kind = gfx::texture::Kind::D2(width, height, aa);
		let tex = self.create_texture(
			kind,
			1,
			gfx::memory::Bind::SHADER_RESOURCE | gfx::memory::Bind::DEPTH_STENCIL,
			gfx::memory::Usage::Data,
			Some(<formats::RenderDepthFormat as gfx::format::Formatted>::get_format().1),
		)?;
		let resource = self.view_texture_as_shader_resource::<formats::RenderDepthFormat>(
			&tex,
			(0, 0),
			gfx::format::Swizzle::new(),
		)?;
		let target = self.view_texture_as_depth_stencil_trivial(&tex)?;
		Ok((tex, resource, target))
	}

	fn create_gtk_compatible_render_target(
		&mut self,
		aa: gfx::texture::AaMode,
		width: gfx::texture::Size,
		height: gfx::texture::Size,
	) -> Result<formats::RenderSurface<R>> {
		let kind = gfx::texture::Kind::D2(width, height, aa);
		let tex = self.create_texture(
			kind,
			1,
			gfx::memory::Bind::SHADER_RESOURCE | gfx::memory::Bind::RENDER_TARGET,
			gfx::memory::Usage::Data,
			Some(<formats::RenderColorFormat as gfx::format::Formatted>::get_format().1),
		)?;
		let hdr_srv = self.view_texture_as_shader_resource::<formats::RenderColorFormat>(
			&tex,
			(0, 0),
			gfx::format::Swizzle::new(),
		)?;
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

fn dynamic_library_get_proc_addr(s: &str) -> *const std::ffi::c_void {
	unsafe {
		match shared_library::dynamic_library::DynamicLibrary::open(None)
			.unwrap()
			.symbol(s)
		{
			Ok(v) => {
				//println!("Loaded {} as {:?}", s, v);
				v
			}
			Err(e) => {
				println!("{:?}", e);
				ptr::null()
			}
		}
	}
}

pub fn epoxy_get_proc_addr(s: &str) -> *const std::ffi::c_void {
	// Workaround for missing functions
	let s = match s {
		"glBlendEquationSeparateiARB" => "glBlendEquationSeparatei",
		"glBlendEquationiARB" => "glBlendEquationi",
		"glBlendFuncSeparateiARB" => "glBlendFuncSeparatei",
		"glBlendFunciARB" => "glBlendFunci",

		_ => s,
	};

	let v = epoxy::get_proc_addr(s);
	if (v.is_null()) {
		println!("Function {} is missing {:?}", s, v);
	}
	v
}

pub fn load() {
	epoxy::load_with(dynamic_library_get_proc_addr);
	gl::load_with(epoxy_get_proc_addr);
}

#[derive(Clone, Copy, Debug)]
pub enum GlRenderCallbackStatus {
	Ok,
	NoFlush,
}

pub trait GlRenderCallback {
	fn render(
		&mut self,
		width: i32,
		height: i32,
		device: &mut GlDevice,
		factory: &mut GlFactory,
		encoder: &mut GlEncoder,
		frame_buffer: &GlFrameBuffer,
		depth_buffer: &GlDepthBuffer,
	) -> GlRenderCallbackStatus;
}

impl GlGfxContext {
	pub fn new(widget_width: i32, widget_height: i32) -> Result<GlGfxContext> {
		Self::new_with_loader(widget_width, widget_height, &epoxy_get_proc_addr)
	}

	pub fn new_with_loader(
		widget_width: i32,
		widget_height: i32,
		get_proc_addr: &Fn(&str) -> *const std::ffi::c_void,
	) -> Result<GlGfxContext> {
		use self::FactoryExt;

		let (device, mut factory) = gfx_device_gl::create(get_proc_addr);
		let encoder = factory.create_command_buffer().into();
		let width = widget_width as gfx::texture::Size;
		let height = widget_height as gfx::texture::Size;
		let (frame_buffer_source, frame_buffer, depth_buffer) =
			factory.create_gtk_compatible_targets(width, height)?;

		Ok(GfxContext {
			device,
			factory,
			encoder,
			width,
			height,
			frame_buffer_source,
			frame_buffer,
			depth_buffer,
		})
	}

	pub fn encoder_mut(&mut self) -> &mut GlEncoder {
		&mut self.encoder
	}

	pub fn device_mut(&mut self) -> &mut GlDevice {
		&mut self.device
	}

	pub fn factory_mut(&mut self) -> &mut GlFactory {
		&mut self.factory
	}

	pub fn size(&self) -> (gfx::texture::Size, gfx::texture::Size) {
		(self.width, self.height)
	}

	pub fn resize(&mut self, widget_width: i32, widget_height: i32) -> Result<()> {
		let new_width = widget_width as gfx::texture::Size;
		let new_height = widget_height as gfx::texture::Size;
		if new_width != self.width || new_height != self.height {
			let (frame_buffer_source, frame_buffer, depth_buffer) = self
				.factory
				.create_gtk_compatible_targets(new_width, new_height)?;

			self.width = new_width;
			self.height = new_height;
			self.frame_buffer_source = frame_buffer_source;
			self.frame_buffer = frame_buffer;
			self.depth_buffer = depth_buffer;
		}

		Ok(())
	}

	pub fn with_gfx<R>(&mut self, render_callback: &mut R)
	where
		R: GlRenderCallback,
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
			self.width as i32,
			self.height as i32,
			&mut self.device,
			&mut self.factory,
			&mut self.encoder,
			&self.frame_buffer,
			&self.depth_buffer,
		);

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
				self.width as i32,
				self.height as i32,
				0,
				0,
				self.width as i32,
				self.height as i32,
				gl::COLOR_BUFFER_BIT,
				gl::NEAREST,
			);
			gl::Flush();
		}
		self.cleanup();
	}

	fn cleanup(&mut self) {
		use gfx::Device;
		self.device.cleanup();
	}
}
