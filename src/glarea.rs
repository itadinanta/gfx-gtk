// copied (and adapted) from https://github.com/mjkoo/examples/blob/glium/src/glarea.rs
//! # GLArea Sample
//!
//! This sample demonstrates how to use GLAreas and OpenGL

#[macro_use]
extern crate glium;

// make moving clones into closures more convenient
macro_rules! clone {
    ($($n:ident),+; || $body:block) => (
        {
            $( let $n = $n.clone(); )+
            move || { $body }
        }
    );
    ($($n:ident),+; |$($p:ident),+| $body:block) => (
        {
            $( let $n = $n.clone(); )+
            move |$($p),+| { $body }
        }
    );
}
mod example {
	extern crate gl;
	extern crate epoxy;
	extern crate gtk;
	extern crate libc;
	extern crate shared_library;

	use std::cell::RefCell;
	use std::ptr;
	use std::rc::Rc;

	use gtk::Inhibit;
	use gtk::traits::*;
	use gtk::{GLArea, Window};

	use glium;
	use glium::Surface;

	use self::shared_library::dynamic_library::DynamicLibrary;

	pub fn main() {
		if gtk::init().is_err() {
			println!("Failed to initialize GTK.");
			return;
		}

		let window = Window::new(gtk::WindowType::Toplevel);
		let glarea = GLArea::new();

		window.connect_delete_event(|_, _| {
			gtk::main_quit();
			Inhibit(false)
		});

		epoxy::load_with(|s| unsafe {
			match DynamicLibrary::open(None).unwrap().symbol(s) {
				Ok(v) => v,
				Err(_) => ptr::null(),
			}
		});

		struct Backend {
			glarea: GLArea,
		}

		unsafe impl glium::backend::Backend for Backend {
			fn swap_buffers(&self) -> Result<(), glium::SwapBuffersError> {
				Ok(())
			}

			unsafe fn get_proc_address(&self, symbol: &str) -> *const std::ffi::c_void {
				epoxy::get_proc_addr(symbol)
			}

			fn get_framebuffer_dimensions(&self) -> (u32, u32) {
				(
					self.glarea.get_allocated_width() as u32,
					self.glarea.get_allocated_height() as u32,
				)
			}

			fn is_current(&self) -> bool {
				unsafe { self.make_current() };
				true
			}

			unsafe fn make_current(&self) {
				if self.glarea.get_realized() {
					self.glarea.make_current();
				}
			}
		}

		struct Facade {
			context: Rc<glium::backend::Context>,
		}

		impl glium::backend::Facade for Facade {
			fn get_context(&self) -> &Rc<glium::backend::Context> {
				&self.context
			}
		}

		impl Facade {
			fn draw(&self) -> glium::Frame {
				glium::Frame::new(
					self.context.clone(),
					self.context.get_framebuffer_dimensions(),
				)
			}
		}

		#[derive(Copy, Clone)]
		struct Vertex {
			position: [f32; 2],
			color: [f32; 3],
		}

		implement_vertex!(Vertex, position, color);

		struct State {
			display: Facade,
			vertex_buffer: glium::VertexBuffer<Vertex>,
			indices: glium::index::NoIndices,
			program: glium::program::Program,
		}

		let state: Rc<RefCell<Option<State>>> = Rc::new(RefCell::new(None));

		glarea.connect_realize(clone!(glarea, state; |_widget| {
            let mut state = state.borrow_mut();
			gl::load_with(epoxy::get_proc_addr);
			let vendor = unsafe { gl::GetString(gl::VENDOR) };
			println!("Vendor ptr {:?}", vendor);
            let display = Facade {
                context: unsafe {
                    glium::backend::Context::new(
                        Backend {
                            glarea: glarea.clone(),
                        }, true, Default::default())
                }.unwrap(),
            };
			let vendor = unsafe { gl::GetString(gl::VENDOR) };
			println!("Vendor ptr {:?}", vendor);
			
            let vertices = vec![
                Vertex{ position: [0.0, 0.5], color: [1.0, 0.0, 0.0] },
                Vertex{ position: [0.5, -0.5], color: [0.0, 1.0, 0.0] },
                Vertex{ position: [-0.5, -0.5], color: [0.0, 0.0, 1.0] },
            ];


            let vertex_buffer = glium::VertexBuffer::new(&display, &vertices).unwrap();
            let indices = glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList);

            let vert_shader_src = r#"
                #version 140
                in vec2 position;
                in vec3 color;
                out vec3 vertex_color;
                void main() {
                    vertex_color = color;
                    gl_Position = vec4(position, 0.0, 1.0);
                }"#;

            let frag_shader_src = r#"
                #version 140
                in vec3 vertex_color;
                out vec4 color;
                void main() {
                    color = vec4(vertex_color, 1.0);
                }"#;

            let program = glium::Program::from_source(&display, vert_shader_src,
                                                      frag_shader_src, None).unwrap();

            *state = Some(State {
                display: display,
                vertex_buffer: vertex_buffer,
                indices: indices,
                program: program,
            });
        }));

		glarea.connect_unrealize(clone!(state; |_widget| {
            let mut state = state.borrow_mut();
            *state = None;
        }));

		glarea.connect_render(clone!(state; |_glarea, _glctx| {
            let state = state.borrow();
            let state = state.as_ref().unwrap();

            let mut target = state.display.draw();
            target.clear_color(0.3, 0.3, 0.3, 1.0);
            target.draw(&state.vertex_buffer, &state.indices, &state.program,
                        &glium::uniforms::EmptyUniforms, &Default::default()).unwrap();
            target.finish().unwrap();

            Inhibit(false)
        }));

		window.set_title("GLArea Example");
		window.set_default_size(400, 400);
		window.add(&glarea);

		window.show_all();
		gtk::main();
	}
}


fn main() {
	example::main()
}
