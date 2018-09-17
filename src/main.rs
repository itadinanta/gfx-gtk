extern crate gdk;
extern crate gl;
extern crate glutin;
extern crate gtk;

use glutin::GlContext;
use gtk::traits::*;
use gtk::Inhibit;
use gtk::Window;

pub fn main() {
	if gtk::init().is_err() {
		println!("Failed to initialize GTK.");
		return;
	}

	let window = Window::new(gtk::WindowType::Toplevel);
	let glarea = gtk::GLArea::new();

	// Hack to set up gl
	let events_loop = glutin::EventsLoop::new();
	let dummy_win = glutin::WindowBuilder::new().with_visibility(false);
	let context = glutin::ContextBuilder::new();
	let gl_window = glutin::GlWindow::new(dummy_win, context, &events_loop).unwrap();
	gl::load_with(|s| gl_window.get_proc_address(s) as *const _);

	window.connect_delete_event(|_, _| {
		gtk::main_quit();
		Inhibit(false)
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
