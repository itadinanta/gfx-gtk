extern crate epoxy;
extern crate gdk;
extern crate gfx;
extern crate gl;
extern crate gtk;
extern crate libc;
extern crate shared_library;

use gfx::format::Formatted;
use gfx::memory::Typed;
use gfx::{format, handle, texture, Device, Encoder, Factory};
use gfx_device_gl;
use gtk::traits::*;
use gtk::Inhibit;
use gtk::Window;
use std::cell::RefCell;
use std::ptr;
use std::rc::Rc;

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
	device: D,
	factory: F,
	encoder: Encoder<D::Resources, D::CommandBuffer>,
	frame_buffer: gfx::handle::RenderTargetView<D::Resources, ScreenColorFormat>,
	depth_buffer: gfx::handle::DepthStencilView<D::Resources, ScreenDepthFormat>,
	background_color: Rgba,
	background_depth: Depth,
}

type GlDevice = gfx_device_gl::Device;
type GlFactory = gfx_device_gl::Factory;
type GlCommandBuffer = gfx_device_gl::CommandBuffer;

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
		self.encoder
			.clear_depth(&self.depth_buffer, self.background_depth);
		self.encoder.flush(&mut self.device);
		self.device.cleanup();
	}
}

type GlGfxContext = GfxContext<GlDevice, GlFactory>;

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

		// create the main color/depth targets
		//let ptr = unsafe { gl::GetString(gl::VENDOR) };
		let dim = (
			allocation.width as u16,
			allocation.height as u16,
			1,
			gfx::texture::AaMode::Single,
		);

		let (device, mut factory) = gfx_device_gl::create(get_proc_addr);
		let encoder = factory.create_command_buffer().into();
		let color_format = ScreenColorFormat::get_format();
		let depthstencil_format = ScreenDepthFormat::get_format();
		let (frame_buffer, depth_buffer) =
			gfx_device_gl::create_main_targets_raw(dim, color_format.0, depthstencil_format.0);

		*gfx_context_clone.borrow_mut() = Some(GlGfxContext {
			device,
			factory,
			encoder,
			frame_buffer: gfx::memory::Typed::new(frame_buffer),
			depth_buffer: gfx::memory::Typed::new(depth_buffer),
			background_color: [1., 1., 1., 1.],
			background_depth: 1.,
		});
	});

	let gfx_context_clone = gfx_context.clone();
	glarea.connect_render(move |_, _| {
		unsafe {
			gl::ClearColor(1.0, 0.0, 0.0, 1.0);
			gl::Clear(gl::COLOR_BUFFER_BIT);

			gl::Flush();
		};
		
		if let Some(ref mut context) = *gfx_context_clone.borrow_mut() {
			context.render();
		}


		Inhibit(false)
	});

	window.set_title("GLArea Example");
	window.set_default_size(400, 400);
	window.add(&glarea);

	window.show_all();
	gtk::main();
}
