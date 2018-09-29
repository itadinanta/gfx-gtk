extern crate epoxy;
extern crate gdk;
#[macro_use]
extern crate gfx;
extern crate cgmath;
extern crate gl;
extern crate gtk;
extern crate libc;
extern crate shared_library;

use gfx::traits::FactoryExt;
use gfx_gtk::GlGfxContext;
use gtk::traits::*;
use gtk::{Inhibit, ObjectExt, Window};
use std::cell::RefCell;
use std::rc::Rc;

pub type PrimitiveIndex = i16;
pub type VertexIndex = u16;

const COLOR_RED: gfx_gtk::Rgba = [1., 0., 0., 1.];
const COLOR_GREEN: gfx_gtk::Rgba = [0., 1., 0., 1.];
const COLOR_BLUE: gfx_gtk::Rgba = [0., 0., 1., 1.];
const COLOR_MAGENTA: gfx_gtk::Rgba = [1., 0., 1., 1.];
const COLOR_WHITE: gfx_gtk::Rgba = [1., 1., 1., 1.];

gfx_defines!(
	vertex Vertex {
		pos: [f32; 3] = "a_Pos",
		color: [f32; 4] = "a_Color",
	}

	constant CameraArgs {
		proj: [[f32; 4]; 4] = "u_Proj",
		view: [[f32; 4]; 4] = "u_View",
	}

	constant ModelArgs {
		transform: [[f32; 4]; 4] = "u_Model",
	}

	pipeline unlit {
		vbuf: gfx::VertexBuffer<Vertex> = (),
		camera: gfx::ConstantBuffer<CameraArgs> = "cb_CameraArgs",
		model: gfx::ConstantBuffer<ModelArgs> = "cb_ModelArgs",
		color_target: gfx::BlendTarget <gfx_gtk::formats::RenderColorFormat> = ("o_Color", gfx::state::ColorMask::all(), gfx::preset::blend::ALPHA),
		depth_target: gfx::DepthTarget <gfx_gtk::formats::RenderDepthFormat> = gfx::preset::depth::LESS_EQUAL_WRITE,
	}
);

struct SimpleRenderCallback {
	model_yaw: cgmath::Deg<f32>,

	vertex_buffer: gfx::handle::Buffer<gfx_gtk::GlResources, Vertex>,
	index_buffer: gfx::Slice<gfx_gtk::GlResources>,
	pso: gfx::pso::PipelineState<gfx_gtk::GlResources, unlit::Meta>,
	camera: gfx::handle::Buffer<gfx_gtk::GlResources, CameraArgs>,
	model: gfx::handle::Buffer<gfx_gtk::GlResources, ModelArgs>,
	clear_color: gfx_gtk::Rgba,
	clear_depth: f32,
}

impl Vertex {
	fn new(x: f32, y: f32, color: gfx_gtk::Rgba) -> Self {
		Vertex {
			pos: [x, y, 0.],
			color,
		}
	}
}

const VERTEX_SHADER: &str = r"
// unlit.vert
#version 150 core

layout (std140) uniform cb_CameraArgs {
	uniform mat4 u_Proj;
	uniform mat4 u_View;
};

layout (std140) uniform cb_ModelArgs {
    mat4 u_Model;
};

in vec3 a_Pos;
in vec4 a_Color;

out VertexData {
	vec4 Position;
	vec4 Color;
}v_Out;

void main() {
	v_Out.Position = u_Model * vec4(a_Pos, 1.0);
	v_Out.Color = a_Color;
	gl_Position = u_Proj * u_View * v_Out.Position;
}
";

const PIXEL_SHADER: &str = r"
// unlit.shader
#version 150 core


in VertexData {
	vec4 Position;
	vec4 Color;
}v_In;

out vec4 o_Color;

void main() {
	o_Color = v_In.Color;
}
";

impl SimpleRenderCallback {
	fn new(factory: &mut gfx_gtk::GlFactory) -> gfx_gtk::Result<Self> {
		let vertices = vec![
			Vertex::new(-1., -1., COLOR_RED),
			Vertex::new(-1., 1., COLOR_GREEN),
			Vertex::new(1., 1., COLOR_BLUE),
			Vertex::new(1., -1., COLOR_MAGENTA),
		];

		let indices = vec![0u16, 1, 2, 2, 3, 0];

		let (vertex_buffer, index_buffer) =
			factory.create_vertex_buffer_with_slice(vertices.as_slice(), indices.as_slice());

		let camera = factory.create_constant_buffer(1);
		let model = factory.create_constant_buffer(1);
		let pso = factory
			.create_pipeline_simple(
				VERTEX_SHADER.as_bytes(),
				PIXEL_SHADER.as_bytes(),
				unlit::new(),
			).unwrap();

		Ok(SimpleRenderCallback {
			model_yaw: cgmath::Deg(0.),
			vertex_buffer,
			index_buffer,
			camera,
			model,
			pso,
			clear_color: COLOR_WHITE,
			clear_depth: 1.,
		})
	}
}

impl gfx_gtk::GlRenderCallback for SimpleRenderCallback {
	fn render(
		&mut self,
		width: i32,
		height: i32,
		device: &mut gfx_gtk::GlDevice,
		_factory: &mut gfx_gtk::GlFactory,
		encoder: &mut gfx_gtk::GlEncoder,
		frame_buffer: &gfx_gtk::GlFrameBuffer,
		depth_buffer: &gfx_gtk::GlDepthBuffer,
	) -> gfx_gtk::GlRenderCallbackStatus {
		encoder.clear_depth(depth_buffer, self.clear_depth);
		encoder.clear(frame_buffer, self.clear_color);

		let aspect_ratio = width as f32 / height as f32;
		let camera_projection = cgmath::perspective(cgmath::Deg(90.0), aspect_ratio, 0.1, 200.0);
		let camera_view = cgmath::Matrix4::look_at(
			cgmath::Point3::new(0., 0., 0.),
			cgmath::Point3::new(0., 0., -1.),
			cgmath::Vector3::new(0., 1.0, 0.),
		);
		let transform = (cgmath::Matrix4::from_translation(cgmath::Vector3::new(0.0, 0.0, -2.0))
			* cgmath::Matrix4::from_angle_y(self.model_yaw)).into();

		encoder.update_constant_buffer(
			&self.camera,
			&CameraArgs {
				proj: camera_projection.into(),
				view: camera_view.into(),
			},
		);
		encoder.update_constant_buffer(&self.model, &ModelArgs { transform });
		encoder.draw(
			&self.index_buffer,
			&self.pso,
			&unlit::Data {
				vbuf: self.vertex_buffer.clone(),
				camera: self.camera.clone(),
				model: self.model.clone(),
				color_target: frame_buffer.clone(),
				depth_target: depth_buffer.clone(),
			},
		);

		encoder.flush(device);
		gfx_gtk::GlRenderCallbackStatus::Ok
	}
}

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
	let render_callback: Rc<RefCell<Option<SimpleRenderCallback>>> = Rc::new(RefCell::new(None));

	glarea.connect_realize({
		let gfx_context = gfx_context.clone();
		let render_callback = render_callback.clone();

		move |widget| {
			if widget.get_realized() {
				widget.make_current();
			}

			let allocation = widget.get_allocation();

			let mut new_context =
				gfx_gtk::GlGfxContext::new(allocation.width, allocation.height).ok();
			if let Some(ref mut new_context) = new_context {
				*render_callback.borrow_mut() =
					SimpleRenderCallback::new(new_context.factory_mut()).ok();
			}
			*gfx_context.borrow_mut() = new_context;
		}
	});

	glarea.connect_resize({
		let gfx_context = gfx_context.clone();
		move |_widget, width, height| {
			if let Some(ref mut context) = *gfx_context.borrow_mut() {
				context.resize(width, height).ok();
			}
		}
	});

	glarea.connect_render({
		let gfx_context = gfx_context.clone();
		let render_callback = render_callback.clone();

		move |_widget, _gl_context| {
			if let Some(ref mut context) = *gfx_context.borrow_mut() {
				if let Some(ref mut render_callback) = *render_callback.borrow_mut() {
					context.with_gfx(render_callback);
				}
			}

			Inhibit(false)
		}
	});

	window.set_title("GLArea with Gtk rendering Example");
	window.set_default_size(400, 400);
	let v_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
	let slider = gtk::Scale::new_with_range(gtk::Orientation::Horizontal, -180.0, 180.0, 0.1);
	slider.set_value(0.0);

	slider.connect_value_changed({
		let render_callback = render_callback.clone();
		let glarea = glarea.downgrade();
		move |widget| {
			if let Some(glarea) = glarea.upgrade() {
				if let Some(ref mut render_callback) = *render_callback.borrow_mut() {
					render_callback.model_yaw = cgmath::Deg(widget.get_value() as f32);
					glarea.queue_draw();
				}
			}
		}
	});

	v_box.pack_start(&slider, false, false, 0);
	v_box.pack_start(&glarea, true, true, 0);

	window.add(&v_box);
	window.show_all();
	gtk::main();
}
