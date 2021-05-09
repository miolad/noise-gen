use wgpu_glyph::{GlyphCruncher, Layout, Section, Text, ab_glyph::{Point, Rect}};
use winit::{event::*, event_loop::*, window::Fullscreen};
use futures::task::SpawnExt;
use std::time::{Duration, Instant};

fn create_swap_chain(device: &wgpu::Device, surface: &wgpu::Surface, window_size: (u32, u32)) -> wgpu::SwapChain {
    device.create_swap_chain(&surface, &wgpu::SwapChainDescriptor {
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
        width: window_size.0,
        height: window_size.1,
        present_mode: wgpu::PresentMode::Fifo
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Winit
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Noise Generator")
        .build(&event_loop)?;

    let mut alt_modifier = false;
    let mut now = Instant::now();
    let mut frame_accumulator = 0usize;
    let mut frame_number = std::num::Wrapping(0u32);
    let mut frame_rate_str = String::new();

    // Initialize WGPU
    let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
    let surface = unsafe { instance.create_surface(&window) };

    let (device, queue) = futures::executor::block_on(async {
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance
        }).await.expect("Adapter creation failed");

        adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Device"),
            features: wgpu::Features::PUSH_CONSTANTS,
            limits: wgpu::Limits {
                max_push_constant_size: 20, // float frame number (used for RNG seed) + 4 floats for text bouding box
                ..Default::default()
            }
        }, None).await.expect("Device creation failed")
    });

    let mut staging_belt = wgpu::util::StagingBelt::new(1024);
    let mut swap_chain = create_swap_chain(&device, &surface, (window.inner_size().width, window.inner_size().height));

    // Prepare glyph_brush
    let roboto_light = wgpu_glyph::ab_glyph::FontArc::try_from_slice(include_bytes!("../assets/fonts/Roboto-Light.ttf"))?;
    let mut glyph_brush = wgpu_glyph::GlyphBrushBuilder::using_font(roboto_light).build(&device, wgpu::TextureFormat::Bgra8UnormSrgb);
    let mut local_pool = futures::executor::LocalPool::new();
    let local_spawner = local_pool.spawner();

    // Initialize the pipeline
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Pipeline layout descriptor"),
        bind_group_layouts: &[],
        push_constant_ranges: &[
            wgpu::PushConstantRange {
                stages: wgpu::ShaderStage::FRAGMENT,
                range: 0..20
            }
        ]
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render pipeline"),
        layout: Some(&pipeline_layout),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        primitive: wgpu::PrimitiveState {
            clamp_depth: false,
            conservative: false,
            cull_mode: Some(wgpu::Face::Back),
            front_face: wgpu::FrontFace::Ccw,
            polygon_mode: wgpu::PolygonMode::Fill,
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None
        },
        vertex: wgpu::VertexState {
            buffers: &[],
            entry_point: "main",
            module: &device.create_shader_module(&wgpu::include_spirv!("../shaders/vert.spv"))
        },
        fragment: Some(wgpu::FragmentState {
            entry_point: "main",
            targets: &[wgpu::ColorTargetState {
                blend: None,
                write_mask: wgpu::ColorWrite::ALL,
                format: wgpu::TextureFormat::Bgra8UnormSrgb
            }],
            module: &device.create_shader_module(&wgpu::include_spirv!("../shaders/frag.spv"))
        })
    });

    // Event Loop
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested, ..
            } => *control_flow = ControlFlow::Exit,

            Event::WindowEvent {
                event: WindowEvent::KeyboardInput {
                    input: KeyboardInput {
                        state: ElementState::Released,
                        virtual_keycode: Some(vkc),
                        ..
                    },
                    ..
                },
                ..
            } => {
                match vkc {
                    VirtualKeyCode::Escape => *control_flow = ControlFlow::Exit,
                    VirtualKeyCode::Return if alt_modifier => {
                        let fs = if window.fullscreen().is_some() { None } else { Some(Fullscreen::Borderless(None)) };
                        window.set_fullscreen(fs);
                    },
                    _ => { }
                }
            },

            Event::WindowEvent {
                event: WindowEvent::Resized(new_size),
                ..
            } if new_size.width != 0 && new_size.height != 0 => {
                swap_chain = create_swap_chain(&device, &surface, (new_size.width, new_size.height));
            },

            Event::WindowEvent {
                event: WindowEvent::ScaleFactorChanged {
                    new_inner_size,
                    ..
                },
                ..
            } if new_inner_size.width != 0 && new_inner_size.height != 0 => {
                swap_chain = create_swap_chain(&device, &surface, (new_inner_size.width, new_inner_size.height));
            },

            Event::WindowEvent {
                event: WindowEvent::ModifiersChanged(state),
                ..
            } => {
                alt_modifier = state.alt();
            },

            Event::MainEventsCleared => {
                // Calculate delta time
                // delta_time = (now.elapsed().as_nanos() as f64) / 1e9f64;
                let elapsed = (now.elapsed().as_nanos() as f64) / 1e09f64;
                if elapsed >= 0.5 {
                    let frame_rate = (frame_accumulator as f64) / elapsed;
                    frame_accumulator = 0;
                    now = Instant::now();

                    frame_rate_str = format!("{:.1} FPS", frame_rate);
                }

                frame_accumulator += 1;
                frame_number += std::num::Wrapping(1u32);
                
                window.request_redraw();
            },

            Event::RedrawRequested(..) => {
                // Don't attempt to render when minimized
                if window.inner_size().width != 0 && window.inner_size().height != 0 {
                    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Main command encoder")
                    });

                    let swap_chain_frame = swap_chain.get_current_frame().expect("Couldn't retrieve current frame from swap chain");

                    let section = Section {
                        screen_position: ((window.inner_size().width / 2) as _, (window.inner_size().height / 2) as _),
                        text: vec![
                            Text::new(&frame_rate_str)
                                .with_color([1.0, 1.0, 1.0, 1.0])
                                .with_scale(40.0)
                        ],
                        ..Default::default()
                    }.with_layout(Layout::default_single_line().h_align(wgpu_glyph::HorizontalAlign::Center).v_align(wgpu_glyph::VerticalAlign::Center));
                    
                    let bounds = glyph_brush.glyph_bounds(&section).unwrap_or(Rect {
                        min: Point::from(section.screen_position),
                        max: Point::from(section.screen_position)
                    });

                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        color_attachments: &[
                            wgpu::RenderPassColorAttachment {
                                view: &swap_chain_frame.output.view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: true
                                }
                            }
                        ],
                        label: Some("Render pass"),
                        depth_stencil_attachment: None
                    });

                    render_pass.set_pipeline(&pipeline);
                    render_pass.set_push_constants(wgpu::ShaderStage::FRAGMENT, 0, bytemuck::cast_slice(&[
                        frame_number.0 as f32,
                        bounds.min.x,
                        bounds.min.y,
                        bounds.max.x,
                        bounds.max.y
                    ]));
                    render_pass.draw(0..6, 0..1);

                    std::mem::drop(render_pass);
                    
                    glyph_brush.queue(section);
                    glyph_brush.draw_queued(
                        &device,
                        &mut staging_belt,
                        &mut encoder,
                        &swap_chain_frame.output.view,
                        window.inner_size().width,
                        window.inner_size().height).expect("Error while drawing text");

                    staging_belt.finish();
                    queue.submit(Some(encoder.finish()));

                    local_spawner.spawn(staging_belt.recall()).expect("Failed to recall staging belt");
                    local_pool.run_until_stalled();
                } else {
                    // Sleep to not eat the CPU, as skipping rendering also unlocks the frame rate
                    std::thread::sleep(Duration::from_millis(100));
                }
            }

            _ => { }
        }
    });
}
