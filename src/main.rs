use anyhow::Result;
use std::num::NonZeroUsize;
use std::sync::Arc;

use vello::kurbo::{Affine, Vec2};
use vello::peniko::{Image, Color, Blob, Format};
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use vello_svg::usvg;
use winit::event::*;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{PhysicalKey, KeyCode};
use winit::window::{Window, WindowBuilder};
use winit::dpi::LogicalSize;
use pollster;


pub enum VehImage {
    Image(Image),
    Svg(usvg::Tree),
}

impl VehImage {
    pub fn size(&self) -> (f64, f64) {
        match self {
            VehImage::Image(image) => (image.width as f64, image.height as f64),
            VehImage::Svg(svg) => {
                let size = svg.size();
                (size.width() as f64, size.height() as f64)
            }
        }
    }
}

// Simple struct to hold the state of the renderer
pub struct ActiveRenderState<'s> {
    // The fields MUST be in this order, so that the surface is dropped before the window
    surface: RenderSurface<'s>,
    window: Arc<Window>,
    transform: Affine,
    prior_position: Option<Vec2>, // for mouse dragging
    mouse_down: bool,
}

enum RenderState<'s> {
    Active(ActiveRenderState<'s>),
    // Cache a window so that it can be reused when the app is resumed after being suspended
    Suspended(Option<Arc<Window>>),
}

fn main() -> Result<()> {
    // Setup a bunch of state:

    // The vello RenderContext which is a global context that lasts for the lifetime of the application
    let mut render_cx = RenderContext::new().unwrap();

    // An array of renderers, one per wgpu device
    let mut renderers: Vec<Option<Renderer>> = vec![];

    // State for our example where we store the winit Window and the wgpu Surface
    let mut render_state = RenderState::Suspended(None);

    // A vello Scene which is a data structure which allows one to build up a description a scene to be drawn
    // (with paths, fills, images, text, etc) which is then passed to a renderer for rendering
    let mut scene = Scene::new();
    let mut subscene: Scene = Scene::new();

    // Create and run a winit event loop
    let event_loop = EventLoop::new()?;
    event_loop
        .run(move |event, event_loop| match event {
            // Setup renderer. In winit apps it is recommended to do setup in Event::Resumed
            // for best cross-platform compatibility
            Event::Resumed => {
                let RenderState::Suspended(cached_window) = &mut render_state else {
                    return;
                };

                // Get the winit window cached in a previous Suspended event or else create a new window
                let window = cached_window
                    .take()
                    .unwrap_or_else(|| create_winit_window(event_loop));

                // Create a vello Surface
                let size = window.inner_size();
                let surface_future = render_cx.create_surface(
                    window.clone(),
                    size.width,
                    size.height,
                    wgpu::PresentMode::AutoVsync,
                );
                let surface = pollster::block_on(surface_future).expect("Error creating surface");

                // Create a vello Renderer for the surface (using its device id)
                renderers.resize_with(render_cx.devices.len(), || None);
                renderers[surface.dev_id]
                    .get_or_insert_with(|| create_vello_renderer(&render_cx, &surface));
  
                let image = open_image();              
                add_image_to_subscene(&mut subscene, &image);

                let (image_width, image_height) = image.size();
                let x_scale = size.width as f64 / image_width as f64;
                let y_scale = size.height as f64 / image_height as f64;
                let scale = x_scale.min(y_scale);

                let transform = Affine::translate(Vec2::new(size.width as f64 / 2., size.height as f64 / 2.)) * Affine::scale(scale) * Affine::translate(-Vec2::new(image_width / 2., image_height / 2.)) * Affine::IDENTITY;
                render_state = RenderState::Active(ActiveRenderState { window, surface, transform, prior_position: None, mouse_down: false});

                event_loop.set_control_flow(ControlFlow::Poll);
            }

            // Save window state on suspend
            Event::Suspended => {
                if let RenderState::Active(state) = &render_state {
                    render_state = RenderState::Suspended(Some(state.window.clone()));
                }
                event_loop.set_control_flow(ControlFlow::Wait);
            }

            Event::WindowEvent {
                ref event,
                window_id,
            } => {
                // Ignore the event (return from the function) if
                //   - we have no render_state
                //   - OR the window id of the event doesn't match the window id of our render_state
                //
                // Else extract a mutable reference to the render state from its containing option for use below
                let render_state = match &mut render_state {
                    RenderState::Active(state) if state.window.id() == window_id => state,
                    _ => return,
                };

                match event {
                    WindowEvent::MouseInput { state, button, .. } => {
                        if button == &MouseButton::Left {
                            render_state.mouse_down = state == &ElementState::Pressed;
                        }
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        const BASE: f64 = 1.05;
                        const PIXELS_PER_LINE: f64 = 20.0;

                        if let Some(prior_position) = render_state.prior_position {
                            let exponent = if let MouseScrollDelta::PixelDelta(delta) = delta {
                                delta.y / PIXELS_PER_LINE
                            } else if let MouseScrollDelta::LineDelta(_, y) = delta {
                                *y as f64
                            } else {
                                0.0
                            };
                            render_state.transform = Affine::translate(prior_position)
                                * Affine::scale(BASE.powf(exponent))
                                * Affine::translate(-prior_position)
                                * render_state.transform;
                            render_state.window.request_redraw();
                        }
                    }
                    WindowEvent::CursorLeft { .. } => {
                        render_state.prior_position = None;
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        let position = Vec2::new(position.x, position.y);
                        if render_state.mouse_down {
                            if let Some(prior) = render_state.prior_position {
                                render_state.transform = Affine::translate(position - prior) * render_state.transform;
                            }
                        }
                        render_state.prior_position = Some(position);
                        render_state.window.request_redraw();
                    }
                    // Exit the event loop when a close is requested (e.g. window's close button is pressed)
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                state: ElementState::Pressed,
                                physical_key: PhysicalKey::Code(keycode),
                                ..
                            },
                        ..
                    } => {
                        match keycode {
                            KeyCode::Escape  => event_loop.exit(),
                            KeyCode::ArrowUp | KeyCode::KeyK => {
                                render_state.transform = render_state.transform * Affine::translate((0.0, -10.0));
                                render_state.window.request_redraw();
                            }
                            KeyCode::ArrowDown | KeyCode::KeyJ => {
                                render_state.transform = render_state.transform * Affine::translate((0.0, 10.0));
                                render_state.window.request_redraw();
                            }
                            KeyCode::ArrowLeft | KeyCode::KeyH => {
                                render_state.transform = render_state.transform * Affine::translate((-10.0, 0.0));
                                render_state.window.request_redraw();
                            }
                            KeyCode::ArrowRight | KeyCode::KeyL => {
                                render_state.transform = render_state.transform * Affine::translate((10.0, 0.0));
                                render_state.window.request_redraw();
                            }
                            _ => {}
                        }   
                    }
                    WindowEvent::CloseRequested => event_loop.exit(),
                    WindowEvent::Resized(_size) => {
                        let size = render_state.window.inner_size();
                        render_cx.resize_surface(
                            &mut render_state.surface,
                            size.width,
                            size.height,
                        );
                        render_state.window.request_redraw();
                    }

                    // This is where all the rendering happens
                    WindowEvent::RedrawRequested => {
                        // Empty the scene of objects to draw. You could create a new Scene each time, but in this case
                        // the same Scene is reused so that the underlying memory allocation can also be reused.
                        scene.reset();

                        scene.append(&mut subscene, Some(render_state.transform));
                        // Get the RenderSurface (surface + config)
                        let surface = &render_state.surface;

                        // Get the window size
                        let width = surface.config.width;
                        let height = surface.config.height;

                        // Get a handle to the device
                        let device_handle = &render_cx.devices[surface.dev_id];

                        // Get the surface's texture
                        let surface_texture = surface
                            .surface
                            .get_current_texture()
                            .expect("failed to get surface texture");

                        // Render to the surface's texture
                        renderers[surface.dev_id]
                            .as_mut()
                            .unwrap()
                            .render_to_surface(
                                &device_handle.device,
                                &device_handle.queue,
                                &scene,
                                &surface_texture,
                                &vello::RenderParams {
                                    base_color: Color::BLACK, // Background color
                                    width,
                                    height,
                                    antialiasing_method: AaConfig::Msaa16,
                                },
                            )
                            .expect("failed to render to surface");

                        // Queue the texture to be presented on the surface
                        surface_texture.present();

                        device_handle.device.poll(wgpu::Maintain::Poll);
                    }
                    _ => {}
                }
            }
            _ => {}
        })
        .expect("Couldn't run event loop");
    Ok(())
}

/// Helper function that creates a Winit window and returns it (wrapped in an Arc for sharing between threads)
fn create_winit_window(event_loop: &winit::event_loop::EventLoopWindowTarget<()>) -> Arc<Window> {
    Arc::new(
        WindowBuilder::new()
            .with_inner_size(LogicalSize::new(1044, 800))
            .with_resizable(true)
            .with_title("veh")
            .build(event_loop)
            .unwrap(),
    )
}

/// Helper function that creates a vello `Renderer` for a given `RenderContext` and `RenderSurface`
fn create_vello_renderer(render_cx: &RenderContext, surface: &RenderSurface) -> Renderer {
    Renderer::new(
        &render_cx.devices[surface.dev_id].device,
        RendererOptions {
            surface_format: Some(surface.format),
            use_cpu: false,
            antialiasing_support: vello::AaSupport::all(),
            num_init_threads: NonZeroUsize::new(1),
        },
    )
    .expect("Couldn't create renderer")
}


fn open_image() -> VehImage {
    let path = std::env::args().nth(1).expect("no path given");
    let valid_formats = vec!["svg", "png", "jpg", "jpeg", "bmp", "gif", "ico", "tiff", "webp"];
    let format = path.split('.').last().expect("no format given");
    if !valid_formats.contains(&format) {
        panic!("invalid format given");
    }

    if format == "svg" {
        let contents = &std::fs::read_to_string(path).expect("read svg failed");
        let fontdb = usvg::fontdb::Database::new();
        let svg = usvg::Tree::from_str(contents, &usvg::Options::default(), &fontdb)
            .expect("failed to parse svg file");
        VehImage::Svg(svg)
    } else {
        let image = image::io::Reader::open(path).expect("open image failed").decode().expect("decode image failed");

        let width = image.width();
        let height = image.height();
        let data = Arc::new(image.into_rgba8().into_vec());
        let blob = Blob::new(data);
        VehImage::Image(Image::new(blob, Format::Rgba8, width, height))
    }
}

fn add_image_to_subscene(scene: &mut Scene, image: &VehImage) -> () {
    match image {
        VehImage::Image(image) => {
            scene.draw_image(&image, Affine::IDENTITY);
        }
        VehImage::Svg(svg) => {
            vello_svg::render_tree(scene, &svg);
        }
    }
}