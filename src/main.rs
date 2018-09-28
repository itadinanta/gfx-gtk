extern crate epoxy;
extern crate gdk;
extern crate gfx;
extern crate gl;
extern crate gtk;
extern crate libc;
extern crate shared_library;

use gdk::GLContextExt;
use gdk::WindowExt;
use gfx::format::Formatted;
use gfx::memory::Typed;
use gfx::traits::FactoryExt;
use gfx::{format, handle, texture, Device, Encoder, Factory};
use gfx_device_gl;
use gtk::traits::*;
use gtk::Inhibit;
use gtk::Window;
use std::cell::RefCell;
use std::ptr;
use std::rc::Rc;

mod formats {
	use gfx;

	pub type Float4 = [f32; 4];
	pub type Rgba = Float4;
	pub type Float = f32;
	pub type RenderColorChannels = gfx::format::R8_G8_B8_A8;
	pub type RenderColorFormat = (RenderColorChannels, gfx::format::Unorm);
	pub type RenderDepthFormat = gfx::format::Depth;

	pub type RenderSurface<R> = (
		gfx::handle::Texture<R, RenderColorChannels>,
		gfx::handle::ShaderResourceView<R, Float4>,
		gfx::handle::RenderTargetView<R, RenderColorFormat>,
	);

	pub type DepthSurface<R> = (
		gfx::handle::Texture<R, gfx::format::D24>,
		gfx::handle::ShaderResourceView<R, Float>,
		gfx::handle::DepthStencilView<R, RenderDepthFormat>,
	);

	pub type RenderSurfaceWithDepth<R> = (
		gfx::handle::ShaderResourceView<R, Float4>,
		gfx::handle::RenderTargetView<R, RenderColorFormat>,
		gfx::handle::DepthStencilView<R, RenderDepthFormat>,
	);

	//pub const MSAA_MODE: gfx::texture::AaMode = gfx::texture::AaMode::Multi(4);
	pub const MSAA_MODE: gfx::texture::AaMode = gfx::texture::AaMode::Single;
}

pub type ScreenColorChannels = gfx::format::R8_G8_B8_A8;
// Srgba8 broken on Linux
pub type ScreenColorFormat = (ScreenColorChannels, gfx::format::Unorm);
// Srgba8 broken on Linux
pub type ScreenDepthFormat = gfx::format::Depth;

type Rgba = [f32; 4];
type Depth = f32;

struct GfxContext<D, F>
where
	D: gfx::Device,
	F: gfx::Factory<D::Resources>,
{
	//	gl_context: gdk::GLContext,
	device: D,
	factory: F,
	encoder: Encoder<D::Resources, D::CommandBuffer>,
	frame_buffer_source: gfx::handle::ShaderResourceView<D::Resources, [f32; 4]>,
	frame_buffer: gfx::handle::RenderTargetView<D::Resources, ScreenColorFormat>,
	depth_buffer: gfx::handle::DepthStencilView<D::Resources, ScreenDepthFormat>,
	background_color: Rgba,
	background_depth: Depth,
}

type GlDevice = gfx_device_gl::Device;
type GlFactory = gfx_device_gl::Factory;
type GlCommandBuffer = gfx_device_gl::CommandBuffer;
type GlResources = <GlDevice as gfx::Device>::Resources;

pub trait Renderer {
	fn render(&mut self);
}

impl<D, F> Renderer for GfxContext<D, F>
where
	D: gfx::Device,
	F: Factory<D::Resources>,
{
	fn render(&mut self) {
		self.encoder
			.clear(&self.frame_buffer, self.background_color);
		//		self.encoder
		//			.clear_depth(&self.depth_buffer, self.background_depth);
		self.encoder.flush(&mut self.device);
		self.device.cleanup();
	}
}

type GlGfxContext = GfxContext<GlDevice, GlFactory>;

#[derive(Debug)]
pub enum RenderError {
	Shader(String),
	PrimitiveIndexOverflow,
}

pub type Result<T> = std::result::Result<T, RenderError>;

impl<T: std::fmt::Display> std::convert::From<T> for RenderError {
	fn from(e: T) -> Self {
		RenderError::Shader(e.to_string())
	}
}

trait RenderFactoryExt<R: gfx::Resources>: gfx::traits::FactoryExt<R> {
	fn create_surfaces(
		&mut self,
		width: gfx::texture::Size,
		height: gfx::texture::Size,
	) -> Result<formats::RenderSurfaceWithDepth<R>> {
		let (_, color_resource, color_target) =
			self.create_msaa_render_target(formats::MSAA_MODE, width, height)?;
		let (_, _, depth_target) = self.create_msaa_depth(formats::MSAA_MODE, width, height)?;
		Ok((color_resource, color_target, depth_target))
	}

	fn create_msaa_depth(
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
			Some(gfx::format::ChannelType::Float),
		)?;
		let resource = self.view_texture_as_shader_resource::<formats::RenderDepthFormat>(
			&tex,
			(0, 0),
			gfx::format::Swizzle::new(),
		)?;
		let target = self.view_texture_as_depth_stencil_trivial(&tex)?;
		Ok((tex, resource, target))
	}

	fn create_msaa_render_target(
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
			Some(gfx::format::ChannelType::Unorm),
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

impl<F, R> RenderFactoryExt<R> for F
where
	F: Factory<R>,
	R: gfx::Resources,
{
}

pub fn main() {
	if gtk::init().is_err() {
		println!("Failed to initialize GTK.");
		return;
	}

	let window = Window::new(gtk::WindowType::Toplevel);
	let glarea = gtk::GLArea::new();

	use self::shared_library::dynamic_library::DynamicLibrary;

	fn raw_get_proc_addr(s: &str) -> *const std::ffi::c_void {
		unsafe {
			match DynamicLibrary::open(None).unwrap().symbol(s) {
				Ok(v) => {
					println!("Loaded {} as {:?}", s, v);
					v
				}
				Err(e) => {
					println!("{:?}", e);
					ptr::null()
				}
			}
		}
	};

	epoxy::load_with(raw_get_proc_addr);

	fn get_proc_addr(s: &str) -> *const std::ffi::c_void {
		let v = epoxy::get_proc_addr(s);
		println!("Loaded {} as {:?}", s, v);
		v
	};

	gl::load_with(get_proc_addr);

	window.connect_delete_event(|_, _| {
		gtk::main_quit();
		Inhibit(false)
	});

	let gfx_context: Rc<RefCell<Option<GlGfxContext>>> = Rc::new(RefCell::new(None));

	let gfx_context_clone = gfx_context.clone();
	glarea.connect_realize(move |widget| {
		if widget.get_realized() {
			widget.make_current();
		}

		let allocation = widget.get_allocation();

		let (mut device, mut factory) = gfx_device_gl::create(get_proc_addr);
		let encoder = factory.create_command_buffer().into();
		let (frame_buffer_source, frame_buffer, depth_buffer) = factory
			.create_surfaces(allocation.width as u16, allocation.height as u16)
			.unwrap();
		//gfx_device_gl::create_main_targets_raw(dim, color_format.0, depthstencil_format.0);
		//		let handle_manager: gfx::handle::Manager<GlResources> = gfx::handle::Manager::new();
		let mut attached_color = 0;
		unsafe {
			device.with_gl(|gl| gl.GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut attached_color))
		};

		//		let frame_buffer = gfx::memory::Typed::new(frame_buffer);
		//		let depth_buffer = gfx::memory::Typed::new(depth_buffer);

		*gfx_context_clone.borrow_mut() = Some(GlGfxContext {
			//			gl_context,
			device,
			factory,
			encoder,
			frame_buffer_source,
			frame_buffer,
			depth_buffer,
			background_color: [1., 1., 0.5, 1.],
			background_depth: 1.,
		});
	});

	let gfx_context_clone = gfx_context.clone();
	glarea.connect_render(move |widget, gl_context| {
		let size = widget.get_allocation();
		if let Some(ref mut context) = *gfx_context_clone.borrow_mut() {
			context.render();
			//			unsafe {
			//				context.device.with_gl(|gl| {
			//					gl.ClearColor(1.0, 0.0, 0.0, 1.0);
			//					gl.Clear(gl::COLOR_BUFFER_BIT);
			//					gl.Flush();
			//				});
			//			}
			//widget.make_current();
			//widget.attach_buffers();
			unsafe {
				epoxy::BindFramebufferEXT(epoxy::READ_FRAMEBUFFER, 1);
				epoxy::BindFramebufferEXT(epoxy::DRAW_FRAMEBUFFER, 2);
				epoxy::FramebufferRenderbufferEXT(
					epoxy::FRAMEBUFFER,
					epoxy::COLOR_ATTACHMENT0,
					epoxy::RENDERBUFFER,
					1,
				);

				epoxy::BlitFramebufferEXT(
					0,
					0,
					size.width,
					size.height,
					0,
					0,
					size.width,
					size.height,
					epoxy::COLOR_BUFFER_BIT,
					epoxy::NEAREST,
				);
				epoxy::Flush();
			}
			//context.gl_context.make_current();
			//
			//			//context.render();
			//			unsafe {
			//				gl::Flush();
			//			}

			//
			//			let attached_color = unsafe {
			//				let mut attached_color = 0;
			//				context.device.with_gl(|gl| {
			//					gl.GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut attached_color)
			//				});
			//				attached_color
			//			};
		}

		Inhibit(false)
	});

	window.set_title("GLArea Example");
	window.set_default_size(400, 400);
	window.add(&glarea);

	window.show_all();
	gtk::main();
}
