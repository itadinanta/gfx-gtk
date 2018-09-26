extern crate epoxy;
extern crate gdk;
extern crate gfx;
extern crate gl;
extern crate gtk;
extern crate libc;
extern crate shared_library;

use std::ptr;

use gfx::format::Formatted;
use gfx::memory::Typed;
use gfx::{format, handle, texture};
use gfx_device_gl as device_gl;
use gfx_device_gl::Resources as R;
use gtk::traits::*;
use gtk::Inhibit;
use gtk::Window;

pub type ScreenColorChannels = gfx::format::R8_G8_B8_A8;
// Srgba8 broken on Linux
pub type ScreenColorFormat = (ScreenColorChannels, gfx::format::Unorm);
// Srgba8 broken on Linux
pub type ScreenDepthFormat = gfx::format::Depth;

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

	glarea.connect_realize(|widget| {
		if widget.get_realized() {
			widget.make_current();
		}

		let allocation = widget.get_allocation();

		// create the main color/depth targets
		let ptr = unsafe { gl::GetString(gl::VENDOR) };
		let dim = (allocation.width as u16, allocation.height as u16, 1, gfx::texture::AaMode::Single);

		let (device, factory) = device_gl::create(get_proc_addr);

		let color_format = ScreenColorFormat::get_format();
		let depthstencil_format = ScreenDepthFormat::get_format();
		let (color_view, ds_view) =
			device_gl::create_main_targets_raw(dim, color_format.0, depthstencil_format.0);
	});

	glarea.connect_render(|_, _| {
		unsafe {
			gl::ClearColor(1.0, 0.0, 0.0, 1.0);
			gl::Clear(gl::COLOR_BUFFER_BIT);

			gl::Flush();
		};

		Inhibit(false)
	});

	window.set_title("GLArea Example");
	window.set_default_size(400, 400);
	window.add(&glarea);

	window.show_all();
	gtk::main();
}
