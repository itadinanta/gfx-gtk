extern crate epoxy;
extern crate gdk;
extern crate gfx;
extern crate gl;
extern crate gtk;
extern crate libc;
extern crate shared_library;

use gfx_gtk::GlGfxContext;
use gtk::traits::*;
use gtk::Inhibit;
use gtk::Window;
use std::cell::RefCell;
use std::rc::Rc;

pub fn main() {
	if gtk::init().is_err() {
		println!("Failed to initialize GTK.");
		return;
	}

	let window = Window::new(gtk::WindowType::Toplevel);
	let glarea = gtk::GLArea::new();

	gfx_gtk::load();

	window.connect_delete_event(|_, _| {
		gtk::main_quit();
		Inhibit(false)
	});

	let gfx_context: Rc<RefCell<Option<GlGfxContext>>> = Rc::new(RefCell::new(None));

	glarea.connect_realize({
		let gfx_context = gfx_context.clone();

		move |widget| {
			if widget.get_realized() {
				widget.make_current();
			}

			let allocation = widget.get_allocation();

			*gfx_context.borrow_mut() =
				gfx_gtk::GlGfxContext::new(allocation.width, allocation.height).ok();
		}
	});

	struct SimpleRenderCallback {
		clear_color: gfx_gtk::Rgba,
		clear_depth: f32,
	}

	impl gfx_gtk::GlRenderCallback for SimpleRenderCallback {
		fn render(
			&mut self,
			_width: i32,
			_height: i32,
			device: &mut gfx_gtk::GlDevice,
			_factory: &mut gfx_gtk::GlFactory,
			encoder: &mut gfx_gtk::GlEncoder,
			frame_buffer: &gfx_gtk::GlFrameBuffer,
			depth_buffer: &gfx_gtk::GlDepthBuffer,
		) -> gfx_gtk::GlRenderCallbackStatus {
			encoder.clear_depth(depth_buffer, self.clear_depth);
			encoder.clear(frame_buffer, self.clear_color);
			encoder.flush(device);
			gfx_gtk::GlRenderCallbackStatus::Ok
		}
	}

	glarea.connect_render({
		let gfx_context = gfx_context.clone();
		move |_widget, _gl_context| {
			if let Some(ref mut context) = *gfx_context.borrow_mut() {
				let mut render_callback = SimpleRenderCallback {
					clear_color: [1., 1., 1., 1.],
					clear_depth: 1.,
				};
				context.with_gfx(&mut render_callback);
			}

			Inhibit(false)
		}
	});

	window.set_title("GLArea Example");
	window.set_default_size(400, 400);
	window.add(&glarea);

	window.show_all();
	gtk::main();
}
