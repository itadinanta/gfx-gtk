//! A bridge lib between [gfx] and [gtk], which allows rendering [gtk::GlArea] content via [gfx]
//! calls and its [gl] backend.
//!
//! Uses [epoxy] for Gl loading, and as such it doesn't require a Gl window/loading management
//! such as `glutin` or `winit`
//!
//! See [https://github.com/itadinanta/gfx-gtk/blob/master/examples/setup.rs] for a simple rendering example.
//!
//! Here's a short broken-down list to get the integration up and running:
//!
//! ## Add the Cargo dependencies
//!
//! ```
//! [dependencies]
//! gfx_gtk = "0.2"
//! ```
//!
//! ## Import crate and packages
//!
//! ```
//! extern crate gfx_gtk;
//!
//! use gfx_gtk::formats;
//! use gfx_gtk::GlRenderContext;
//! ```
//!
//! ## Choose some render formats and AA mode
//!
//! ```
//! const MSAA: gfx::texture::AaMode = formats::MSAA_4X;
//! type RenderColorFormat = formats::DefaultRenderColorFormat;
//! type RenderDepthFormat = formats::DefaultRenderDepthFormat;
//! ```
//!
//! ## Write a render callback
//!
//! You need to implement [GlRenderCallback] and [GlPostprocessCallback] traits (the latter
//! can be made to use the default implementation)
//!
//! ```
//! struct SimpleRenderCallback {
//! 	...
//! }
//!
//! impl gfx_gtk::GlRenderCallback<RenderColorFormat, RenderDepthFormat> for SimpleRenderCallback {
//! 	fn render(
//!			&mut self,
//!			gfx_context: &mut gfx_gtk::GlGfxContext,
//!			viewport: &gfx_gtk::Viewport,
//!			frame_buffer: &gfx_gtk::GlFrameBuffer<RenderColorFormat>,
//!			depth_buffer: &gfx_gtk::GlDepthBuffer<RenderDepthFormat>,
//!		) -> gfx_gtk::Result<gfx_gtk::GlRenderCallbackStatus> {
//! 		gfx_context.encoder.draw(...);
//! 		Ok(gfx_gtk::GlRenderCallbackStatus::Continue)
//! 	}
//! }
//!
//! impl gfx_gtk::GlPostprocessCallback<RenderColorFormat, RenderDepthFormat> for SimpleRenderCallback {}
//! ```
//!
//! ### Load Gl functions
//!
//! ```
//! gfx_gtk::load();
//!
//! ```
//! ### Connect the widget's signals
//!
//! The rendering needs to be driven by a `GlArea` widget because of its ability to create a Gl context.
//!
//! The `realize`, `resize` and `render` signals need to be connected. The [GlRenderContext]
//! and [GlRenderCallback] must be created in the closure that gets attached to `GlArea::connect_realize()` after
//! the `make_current()` call (otherwise it won't be possible to "bind" to the current `GlArea` Gl context
//!
//! ```
//!
//!	let gfx_context: Rc<RefCell<Option<GlRenderContext<RenderColorFormat, RenderDepthFormat>>>> = Rc::new(RefCell::new(None));
//!
//!	let render_callback: Rc<RefCell<Option<SimpleRenderCallback>>> = Rc::new(RefCell::new(None));
//!
//!	let glarea = gtk::GLArea::new();
//!
//!	glarea.connect_realize({
//!		let gfx_context = gfx_context.clone();
//!		let render_callback = render_callback.clone();
//!
//!		move |widget| {
//!			if widget.get_realized() {
//!				widget.make_current();
//!			}
//!
//!			let allocation = widget.get_allocation();
//!
//!			let mut new_context =
//!				gfx_gtk::GlRenderContext::new(
//! 				MSAA,
//! 				allocation.width,
//! 				allocation.height,
//! 				None).ok();
//!			if let Some(ref mut new_context) = new_context {
//!				let ref vp = new_context.viewport();
//!				let ref mut ctx = new_context.gfx_context_mut();
//!				*render_callback.borrow_mut() = SimpleRenderCallback::new(ctx, vp).ok();
//!			}
//!			*gfx_context.borrow_mut() = new_context;
//!		}
//!	});
//!
//!	glarea.connect_resize({
//!		let gfx_context = gfx_context.clone();
//!		let render_callback = render_callback.clone();
//!
//!		move |_widget, width, height| {
//!			if let Some(ref mut context) = *gfx_context.borrow_mut() {
//!				if let Some(ref mut render_callback) = *render_callback.borrow_mut() {
//!					context.resize(width, height, Some(render_callback)).ok();
//!				}
//!			}
//!		}
//!	});
//!
//!	glarea.connect_render({
//!		let gfx_context = gfx_context.clone();
//!		let render_callback = render_callback.clone();
//!
//!		move |_widget, _gl_context| {
//!			if let Some(ref mut context) = *gfx_context.borrow_mut() {
//!				if let Some(ref mut render_callback) = *render_callback.borrow_mut() {
//!					context.with_gfx(render_callback);
//!				}
//!			}
//!
//!			Inhibit(false)
//!		}
//!	});
//! ```
//! After this, every time Gtk refreshes the `GlArea` content, it will invoke the `render_callback` to paint itself.
//!

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

/// Convenience type to express a typical RGBA quantity as [r,g,b,a] f32
pub type Rgba = [f32; 4];
/// Convenience type to express a general purpose vec4 [x,y,z,w] f32
pub type Float4 = [f32; 4];
/// Convenience type to express a floating point depth value as f32
pub type Depth = f32;

/// Contains definitions of the default color and depth formats
/// with an eye on compatibility with the GlArea render targets
pub mod formats {
	use gfx;

	/// Default render format, RGBA8888
	pub type GtkTargetColorFormat = gfx::format::Rgba8;
	/// Default render format [f32;4]
	pub type GtkTargetColorView = <GtkTargetColorFormat as gfx::format::Formatted>::View;
	/// Default render format, RGBA8888
	pub type DefaultRenderColorFormat = gfx::format::Rgba8;
	/// Default depth+stencil format, 24/8
	pub type DefaultRenderDepthFormat = gfx::format::DepthStencil;

	/// Convenience type for return values of functions that create offscreen
	/// render targets
	pub type RenderSurface<R, CF> = (
		gfx::handle::Texture<R, <CF as gfx::format::Formatted>::Surface>,
		gfx::handle::ShaderResourceView<R, <CF as gfx::format::Formatted>::View>,
		gfx::handle::RenderTargetView<R, CF>,
	);

	/// Convenience type for return values of functions that create offscreen
	/// depth targets
	pub type DepthSurface<R, DF> = (
		gfx::handle::Texture<R, <DF as gfx::format::Formatted>::Surface>,
		gfx::handle::ShaderResourceView<R, <DF as gfx::format::Formatted>::View>,
		gfx::handle::DepthStencilView<R, DF>,
	);

	/// Convenience type for return values of functions that create offscreen
	/// depth targets
	pub type RenderSurfaceWithDepth<R, CF, DF> = (
		gfx::handle::ShaderResourceView<R, <CF as gfx::format::Formatted>::View>,
		gfx::handle::RenderTargetView<R, CF>,
		gfx::handle::DepthStencilView<R, DF>,
	);

	/// No MSAA
	pub const MSAA_NONE: gfx::texture::AaMode = gfx::texture::AaMode::Single;
	/// 4x MSAA - other methods can be implemented in the future, this does quite ok
	pub const MSAA_4X: gfx::texture::AaMode = gfx::texture::AaMode::Multi(4);
}

/// Post-processing gfx vertex structure
gfx_vertex_struct!(BlitVertex {
	pos: [f32; 2] = "a_Pos",
	tex_coord: [f32; 2] = "a_TexCoord",
});

/// Post-processing gfx pipeline definitions
gfx_pipeline!(postprocess {
		vbuf: gfx::VertexBuffer<BlitVertex> = (),
		src: gfx::TextureSampler<formats::GtkTargetColorView> = "t_Source",
		dst: gfx::RenderTarget<formats::GtkTargetColorFormat> = "o_Color",
	}
);

#[allow(unused)]
/// A container for a GL device and factory, with a convenience encoder ready to use.
/// Typically, it will be specialised, including a GlDevice and GlFactory
pub struct GfxContext<D, F>
where
	D: gfx::Device,
	F: gfx::Factory<D::Resources>,
{
	/// GFX device
	pub device: D,
	/// GFX factory
	pub factory: F,
	/// Convenience encoder, other encoders can be used by the library client
	/// when appropriate
	pub encoder: gfx::Encoder<D::Resources, D::CommandBuffer>,
}

impl<D, F> GfxContext<D, F>
where
	D: gfx::Device,
	F: gfx::Factory<D::Resources>,
{
	/// creates a Gfx PSO given a vertex/pixel shader pair. The PSO will contain
	/// a MSAA-enabled rasterizer if AaMode is Multi(_)
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

/// a container for the pre-built data and state needed to perform
/// MSAA resolution and sRGB correction in the post-processing stage
pub struct PostprocessContext<D>
where
	D: gfx::Device,
{
	/// a sampler for the source framebuffer
	pub sampler: gfx::handle::Sampler<D::Resources>,
	/// pipeline state object with rasterizer and blit shaders
	pub pso: gfx::PipelineState<D::Resources, postprocess::Meta>,
	/// a single large triangle (vertices) covering the full screen
	pub vbuf: gfx::handle::Buffer<D::Resources, BlitVertex>,
	/// a single large triangle (indices)
	pub ibuf: gfx::Slice<D::Resources>,
}

impl PostprocessContext<GlDevice> {
	/// performs a full screen pass using the original render screen as the source
	/// and the GTK framebuffer as the target, using baked-in settings
	/// TODO: make this easy to override
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

impl<D, F> GfxContext<D, F>
where
	D: gfx::Device,
	F: gfx::Factory<D::Resources>,
{
	/// flushes the command buffer and executes its content
	pub fn flush(&mut self) {
		self.encoder.flush(&mut self.device);
	}
}

#[allow(unused)]
/// Structure encapsulating all the GFX state needed for rendering within
/// a GL context in GTK
pub struct RenderContext<D, F, CF, DF>
where
	D: gfx::Device,
	F: gfx::Factory<D::Resources>,
	CF: gfx::format::Formatted,
{
	/// GFX factory, device and commands
	gfx_context: GfxContext<D, F>,
	/// Describes the gtk GlArea size and AA capability
	viewport: Viewport,
	/// Resources used by the postprocess step
	postprocess_context: PostprocessContext<D>,
	/// Render target, destination of the post-process stage
	postprocess_target: gfx::handle::RenderTargetView<D::Resources, formats::GtkTargetColorFormat>,
	/// Off-screen texture view of the render target, source of the post-process stage
	render_target_source: gfx::handle::ShaderResourceView<D::Resources, CF::View>,
	/// Render target, destination of the main render stage
	render_target: gfx::handle::RenderTargetView<D::Resources, CF>,
	/// Depth buffer, used by the main render stage
	depth_buffer: gfx::handle::DepthStencilView<D::Resources, DF>,
}

/// gfx device, Gl backend
pub type GlDevice = gfx_device_gl::Device;
/// gfx factory, Gl backend
pub type GlFactory = gfx_device_gl::Factory;
/// gfx command buffer, Gl backend
pub type GlCommandBuffer = gfx_device_gl::CommandBuffer;
/// gfx resources, Gl backend
pub type GlResources = <GlDevice as gfx::Device>::Resources;
/// gfx encoder, Gl backend
pub type GlEncoder = gfx::Encoder<GlResources, GlCommandBuffer>;
/// gfx texture source view of the main render target, Gl backend
pub type GlFrameBufferTextureSrc<F> =
	gfx::handle::ShaderResourceView<GlResources, <F as gfx::format::Formatted>::View>;
/// gfx main render target, Gl backend
pub type GlFrameBuffer<CF> = gfx::handle::RenderTargetView<GlResources, CF>;
/// gfx main depth buffer, Gl backend
pub type GlDepthBuffer<DF> = gfx::handle::DepthStencilView<GlResources, DF>;
/// render context, specialized for the gfx Gl backend
pub type GlRenderContext<CF, DF> = RenderContext<GlDevice, GlFactory, CF, DF>;

#[derive(Debug)]
/// Error type for [Result]
pub enum Error {
	/// Used to convert any error into this one by encapsulating the original error into
	/// a string message
	GenericError(String),
}

/// Result which produces an [Error] on failure
pub type Result<T> = std::result::Result<T, self::Error>;

impl<T: std::fmt::Display> std::convert::From<T> for self::Error {
	fn from(e: T) -> Self {
		self::Error::GenericError(e.to_string())
	}
}

/// Extends [gfx::traits::FactoryExt] with utility functions specific to the gfx to gtk integration
pub trait FactoryExt<R: gfx::Resources>: gfx::traits::FactoryExt<R> {
	/// Creates a render target (with its associated texture source view and a depth target
	/// which are resonably compatible with something that we can blit onto a GtkGlView
	/// framebuffer
	/// * `aa` antialiasing mode, currently supported `Single` and `Multi(4)`
	/// * `width` width of the client area of the containing widget
	/// * `height` height of the client area of the containing widget`
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

	/// creates a Gfx PSO given a vertex/pixel shader pair. The PSO will contain
	/// a MSAA-enabled rasterizer if AaMode is Multi(_)
	/// * `aa` antialiasing mode, currently supported `Single` and `Multi(4)`
	/// `vertex_shader` GLSL source code of the vertex shader
	/// `pixel_shader` GLSL source code of the pixel shader
	/// `init` the gfx pipeline initializer for `I`
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

	/// Creates a depth target for the GlArea client area
	/// * `aa` antialiasing mode, currently supported `Single` and `Multi(4)`
	/// * `width` width of the client area of the containing widget
	/// * `height` height of the client area of the containing widget`
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

	/// Creates a render target for the GlArea client area
	/// * `aa` antialiasing mode, currently supported `Single` and `Multi(4)`
	/// * `width` width of the client area of the containing widget
	/// * `height` height of the client area of the containing widget`
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
/// Loads the Gl function pointers via epoxy.
///
/// Functions names are looked up first in the current .exe, and, failing that,
/// in the `libepoxy` dylib - attempting to load `libepoxy-0`, `libepoxy0` and `libepoxy`
///
/// This function needs to be invoked only once, at startup, by the host program.
///
/// Failure to load any function will be silent. Use [debug_load()] for diagnostic output.
///
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

/// Loads the Gl function pointers via epoxy, with some diagnostic output.
///
/// Functions names are looked up first in the current .exe, and, failing that,
/// in the `libepoxy` dylib - attempting to load `libepoxy-0`, `libepoxy0` and `libepoxy`
///
/// This function needs to be invoked only once, at startup, by the host program.
///
/// Will dump to stdout any failure to load a function (for dll or symbol not found) so this
/// is better suited for debugging. Use [load()] instead for production code.
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
/// Hint returned at the end of Render and PostProcess calls.
/// Returning `Skip` at the end of the render pass will bypass
/// the postprocessing stage
pub enum GlRenderCallbackStatus {
	/// Continue onto the next render pass, from Render to Postprocess
	Continue,
	/// Skip the next render passes
	Skip,
}

/// Specialization of the GlRenderContext to be used with a Gl device
pub type GlGfxContext = GfxContext<GlDevice, GlFactory>;
/// Specalization of the GlCallbackContext to be used with a Gl device
pub type GlPostprocessContext = PostprocessContext<GlDevice>;

#[derive(Clone)]
/// Describes the client area of the GlArea being rendered into
pub struct Viewport {
	/// Width of the render target in pixels. This may be larger than the actual client window size.
	pub width: i32,
	/// Height of the render target in pixels. This may be larger than the actual client window size.
	pub height: i32,
	/// Width of the GlArea client in pixels
	pub target_width: i32,
	/// Height of the GlArea client in pixels
	pub target_height: i32,
	/// Antialiasing mode (supported `Single` and `Multi(4)`)
	pub aa: gfx::texture::AaMode,
}

impl Viewport {
	/// The ratio between width and height of the
	pub fn aspect_ratio(&self) -> f32 {
		self.target_width as f32 / self.target_height as f32
	}

	/// Creates a new Viewport from the specified source GlArea size. `width` and `height`
	/// will be determined accordingly and taking into account supersampling if applicable
	/// * `aa` antialiasing mode, currently supported `Single` and `Multi(4)`
	/// * `target_width` width of the client area of the containing widget
	/// * `target_height` height of the client area of the containing widget`
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

	/// Computes the `width` and `height` of the offscreen render and depth target
	/// from the `width` and `height` of the GlArea widget client area, taking into
	/// account the `aa` hint, if the
	/// * `aa` antialiasing mode, currently supported `Single` and `Multi(4)`
	/// * `target_width` width of the client area of the containing widget
	/// * `target_height` height of the client area of the containing widget`
	fn aa_size(aa: gfx::texture::AaMode, target_width: i32, target_height: i32) -> (i32, i32) {
		let (mx, my) = match aa {
			gfx::texture::AaMode::Single => (1, 1),
			// TODO: if we are not implementing supersampling, this is unnecessary
			gfx::texture::AaMode::Multi(_) => (1, 1),
			_ => (0, 0),
		};
		(target_width * mx, target_height * my)
	}
}

/// Implement custom render behaviour for the GlArea
/// * `CF` color format of the offline target
/// * `DF` depth format of the offline target
pub trait GlRenderCallback<CF, DF>
where
	CF: gfx::format::Formatted<View = formats::GtkTargetColorView>,
	CF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
	CF::Surface: gfx::format::RenderSurface + gfx::format::TextureSurface,
	DF: gfx::format::Formatted,
	DF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
	DF::Surface: gfx::format::DepthSurface + gfx::format::TextureSurface,
{
	/// Invoked when the GlArea needs rendering (after an expose or queue_draw event)
	/// * `gfx_context` Gfx device, factory, encoder attached to the current Gl context
	/// * `viewport` size of the GlArea
	/// * `render_target` the offscreen target to render to
	/// * `depth_buffer` the offscreen depth buffer associated to the `render_target`
	///
	/// After rendering, the result should contain either:
	/// *`Ok(Continue)` to proceed with the postprocessing step
	/// *`Ok(Skip)` to blit directly to the GlArea buffer
	/// *`Err(_)` will stop the rendering of the requested frame
	/// Gtk may or may not retain the previous state of the frame
	fn render(
		&mut self,
		gfx_context: &mut GlGfxContext,
		viewport: &Viewport,
		render_target: &GlFrameBuffer<CF>,
		depth_buffer: &GlDepthBuffer<DF>,
	) -> Result<GlRenderCallbackStatus>;

	/// Invoked when the GlArea has been resized
	/// * `gfx_context` Gfx device, factory, encoder attached to the current Gl context
	/// * `viewport` size of the GlArea after resizing
	/// Should return `Continue`
	fn resize(
		&mut self,
		_gfx_context: &mut GlGfxContext,
		_viewport: Viewport,
	) -> Result<GlRenderCallbackStatus> {
		Ok(GlRenderCallbackStatus::Continue)
	}
}

/// Implement custom post-processing behaviour for the GlArea
/// * `CF` color format of the offline target
/// * `DF` depth format of the offline target
pub trait GlPostprocessCallback<CF, DF>
where
	CF: gfx::format::Formatted<View = formats::GtkTargetColorView>,
	CF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
	CF::Surface: gfx::format::RenderSurface + gfx::format::TextureSurface,
	DF: gfx::format::Formatted,
	DF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
	DF::Surface: gfx::format::DepthSurface + gfx::format::TextureSurface,
{
	/// Invoked when the GlArea needs rendering (after an expose or queue_draw event)
	/// * `gfx_context` Gfx device, factory, encoder attached to the current Gl context
	/// * `viewport` size of the GlArea
	/// * `render_target` the offscreen target to render to
	/// * `depth_buffer` the offscreen depth buffer associated to the `render_target`
	/// Returns:
	/// * `Ok(Continue)` will flush the command buffer and complete the frame by blitting to the GlArea
	/// * `Err(_)` will stop the rendering of the requested frame
	/// By default, the post
	fn postprocess(
		&mut self,
		gfx_context: &mut GlGfxContext,
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
		Ok(GlRenderCallbackStatus::Continue)
	}
}

impl<CF, DF> GlRenderContext<CF, DF>
where
	CF: gfx::format::Formatted<View = [f32; 4]>,
	CF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
	CF::Surface: gfx::format::RenderSurface + gfx::format::TextureSurface,
	DF: gfx::format::Formatted,
	DF::Channel: gfx::format::TextureChannel + gfx::format::RenderChannel,
	DF::Surface: gfx::format::DepthSurface + gfx::format::TextureSurface,
{
	/// Creates a new Gfx GlRender context including the Gl Device. The default `epoxy` Gl function pointer
	/// will be used to load the Gl binding.
	/// * `aa` antialiasing mode, currently supported `Single` and `Multi(4)`
	/// * `widget_width` width of the client area of the containing widget
	/// * `widget_height` height of the client area of the containing widget`
	/// * `postprocess_shader` optional buffer containing the source code of the postprocessing pixel shader.
	/// A simple default shader will be used if `None`
	pub fn new(
		aa: gfx::texture::AaMode,
		widget_width: i32,
		widget_height: i32,
		postprocess_shader: Option<&[u8]>,
	) -> Result<GlRenderContext<CF, DF>> {
		Self::new_with_loader(
			aa,
			widget_width,
			widget_height,
			&epoxy::get_proc_addr,
			postprocess_shader,
		)
	}
	/// Creates a new Gfx GlRender context including the Gl Device.
	/// * `aa` antialiasing mode, currently supported `Single` and `Multi(4)`
	/// * `widget_width` width of the client area of the containing widget
	/// * `widget_height` height of the client area of the containing widget`
	/// * `get_proc_addr` the function used to look up the Gl API function pointers (usually `epoxy::get_proc_addr`)
	/// * `postprocess_shader` optional buffer containing the source code of the postprocessing pixel shader.
	/// A simple default shader will be used if `None`
	pub fn new_with_loader(
		aa: gfx::texture::AaMode,
		widget_width: i32,
		widget_height: i32,
		get_proc_addr: &Fn(&str) -> *const std::ffi::c_void,
		postprocess_shader: Option<&[u8]>,
	) -> Result<GlRenderContext<CF, DF>> {
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

		let pixel_shader_code = postprocess_shader.unwrap_or_else(|| match viewport.aa {
			gfx::texture::AaMode::Multi(4) => shaders::POST_PIXEL_SHADER_MSAA_4X.as_bytes(),
			_ => shaders::POST_PIXEL_SHADER.as_bytes(),
		});

		let post_pso = factory.create_pipeline_simple(
			shaders::POST_VERTEX_SHADER.as_bytes(),
			pixel_shader_code,
			postprocess::new(),
		)?;

		let postprocess_context = PostprocessContext {
			vbuf,
			ibuf,
			pso: post_pso,
			sampler: nearest_sampler,
		};

		let gfx_context = GfxContext {
			device,
			factory,
			encoder,
		};

		Ok(RenderContext {
			gfx_context,
			viewport,
			postprocess_context,
			render_target_source,
			render_target,
			depth_buffer,
			postprocess_target,
		})
	}

	/// Returns a reference to the current Gfx context
	pub fn gfx_context_mut(&mut self) -> &mut GlGfxContext {
		&mut self.gfx_context
	}

	/// Returns a copy of the current viewport
	pub fn viewport(&self) -> Viewport {
		self.viewport.clone()
	}

	/// Re-allocates render buffers and textures if the size has changed since last resize or creation of the context
	/// * `widget_width` width of the client area of the containing widget
	/// * `widget_height` height of the client area of the containing widget`
	/// * `render_callback` if `Some(_)`, forwards the resize message to the given RenderCallbak for internal adjustment
	pub fn resize<R>(
		&mut self,
		widget_width: i32,
		widget_height: i32,
		mut render_callback: Option<&mut R>,
	) -> Result<()>
	where
		R: GlRenderCallback<CF, DF>,
	{
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

			if let Some(ref mut render_callback) = render_callback {
				render_callback.resize(&mut self.gfx_context, self.viewport.clone())?;
			};
		}

		Ok(())
	}

	/// Renders on the `GlArea` by invoking the `render` function to write onto the offline render target
	/// and blit the result onto the actual `GlArea` framebuffer, optionally applying an intermediate
	/// `postprocess` step (also customizable).
	/// Also transparently takes care of Gl context and state changes.
	/// * `render_callback` a reference of the render callback implementing the actual drawing
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
		let render_result = GlRenderCallback::render(
			render_callback,
			&mut self.gfx_context,
			&self.viewport,
			&self.render_target,
			&self.depth_buffer,
		);

		let postprocess_result = match render_result {
			Ok(GlRenderCallbackStatus::Continue) => GlPostprocessCallback::postprocess(
				render_callback,
				&mut self.gfx_context,
				&self.postprocess_context,
				&self.viewport,
				&self.render_target_source,
				&self.postprocess_target,
			), // TODO: handle error
			Ok(_) => {
				self.gfx_context.flush();
				Ok(GlRenderCallbackStatus::Skip)
			}
			Err(e) => Err(e),
		};
		if postprocess_result.is_ok() {
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
		}
		self.cleanup();
	}

	fn cleanup(&mut self) {
		use gfx::Device;
		self.gfx_context.device.cleanup();
	}
}
