pub mod renderer;

use std::sync::Arc;

use glam::Vec3;
use renderer::{build_camera_matrix, PartPipeline};
use revival_core::{physics::PhysicsWorld, DataModel};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

pub struct ClientSession {
    pub data_model: DataModel,
    pub physics: PhysicsWorld,
}

impl ClientSession {
    pub fn offline_baseplate() -> Self {
        let data_model = DataModel::new_baseplate();
        let physics = PhysicsWorld::from_data_model(&data_model);
        Self {
            data_model,
            physics,
        }
    }

    pub fn from_place_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let data_model = DataModel::from_place_bytes(bytes)?;
        let physics = PhysicsWorld::from_data_model(&data_model);
        Ok(Self {
            data_model,
            physics,
        })
    }
}

struct GpuState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    pipeline: PartPipeline,
}

pub struct ClientApp {
    session: ClientSession,
    gpu: Option<GpuState>,
    cam_pos: Vec3,
    cam_yaw: f32,
    last_touch: Option<(f64, f64)>,
}

impl ClientApp {
    pub fn new(session: ClientSession) -> Self {
        Self {
            session,
            gpu: None,
            cam_pos: Vec3::new(0.0, 12.0, 28.0),
            cam_yaw: 0.0,
            last_touch: None,
        }
    }

    pub fn auto_frame_camera(&mut self) {
        let visible: Vec<_> = self
            .session
            .data_model
            .parts
            .iter()
            .filter(|p| p.transparency < 0.95 && p.position.length() < 500.0)
            .collect();
        if visible.len() > 10 {
            let sum: Vec3 = visible.iter().map(|p| p.position).sum();
            let center = sum / (visible.len() as f32);
            self.cam_pos = center + Vec3::new(0.0, 65.0, 120.0);
            self.cam_yaw = 0.0;
        }
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = EventLoop::new()?;
        event_loop.run_app(&mut self)?;
        Ok(())
    }

    #[cfg(target_os = "android")]
    pub fn run_android(
        mut self,
        app: winit::platform::android::activity::AndroidApp,
    ) -> anyhow::Result<()> {
        use winit::platform::android::EventLoopBuilderExtAndroid;
        let event_loop = EventLoop::builder().with_android_app(app).build()?;
        event_loop.run_app(&mut self)?;
        Ok(())
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    let data_dir = app.internal_data_path();
    if let Some(dir) = &data_dir {
        let crash_path = dir.join("last_crash.txt");
        let _ = std::fs::remove_file(&crash_path);
        std::panic::set_hook(Box::new(move |info| {
            let _ = std::fs::write(&crash_path, format!("Engine panic: {info}"));
        }));
    }

    let place_bytes = data_dir
        .as_ref()
        .and_then(|dir| std::fs::read(dir.join("pending_place.rbxl")).ok());

    let dir_for_err = data_dir.clone();
    let session = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let mut session = match place_bytes {
                Some(bytes) => match ClientSession::from_place_bytes(&bytes) {
                    Ok(s) => s,
                    Err(e) => {
                        if let Some(dir) = &dir_for_err {
                            let _ = std::fs::write(
                                dir.join("last_crash.txt"),
                                format!("Failed to parse .rbxl: {e:#}"),
                            );
                        }
                        ClientSession::offline_baseplate()
                    }
                },
                None => ClientSession::offline_baseplate(),
            };
            let _ = session.data_model.run_luau_scripts_mut();
            session.physics = PhysicsWorld::from_data_model(&session.data_model);
            session
        })
        .ok()
        .and_then(|h| h.join().ok())
        .unwrap_or_else(ClientSession::offline_baseplate);

    let mut client = ClientApp::new(session);
    client.auto_frame_camera();
    if let Err(e) = client.run_android(app) {
        if let Some(dir) = &data_dir {
            let _ = std::fs::write(dir.join("last_crash.txt"), format!("Engine error: {e:#}"));
        }
    }
}

impl ApplicationHandler for ClientApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("Revival Client (Offline / Online)");
        #[cfg(target_arch = "wasm32")]
        let attrs = {
            use winit::platform::web::WindowAttributesExtWebSys;
            let doc = web_sys::window().unwrap().document().unwrap();
            let canvas = doc
                .get_element_by_id("revival-canvas")
                .and_then(|el| wasm_bindgen::JsCast::dyn_into::<web_sys::HtmlCanvasElement>(el).ok());
            attrs.with_canvas(canvas)
        };

        let Ok(window) = event_loop.create_window(attrs) else {
            return;
        };
        let window = Arc::new(window);
        let size = window.inner_size();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let try_init = |backends: wgpu::Backends| -> Option<GpuState> {
                let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                    backends,
                    ..Default::default()
                });
                let surface = instance.create_surface(window.clone()).ok()?;
                let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..Default::default()
                }))?;

                let desc = wgpu::DeviceDescriptor {
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    ..Default::default()
                };
                let (device, queue) = pollster::block_on(adapter.request_device(&desc, None)).ok()?;

                let caps = surface.get_capabilities(&adapter);
                let format = caps.formats.first().copied()?;
                let alpha_mode = caps.alpha_modes.first().copied()?;
                let config = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format,
                    width: size.width.max(1),
                    height: size.height.max(1),
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode,
                    view_formats: vec![],
                    desired_maximum_frame_latency: 2,
                };
                surface.configure(&device, &config);

                let depth_view = PartPipeline::create_depth_view(&device, config.width, config.height);
                let pipeline = PartPipeline::new(&device, format);

                Some(GpuState {
                    window: window.clone(),
                    surface,
                    device,
                    queue,
                    config,
                    depth_view,
                    pipeline,
                })
            };

            self.gpu = try_init(wgpu::Backends::all()).or_else(|| try_init(wgpu::Backends::GL));
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = size;
            // On wasm32, surface/adapter/device are initialized asynchronously via wasm_bindgen_futures::spawn_local.
            let _ = window;
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.gpu = None;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = new_size.width.max(1);
                    gpu.config.height = new_size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);
                    gpu.depth_view = PartPipeline::create_depth_view(
                        &gpu.device,
                        gpu.config.width,
                        gpu.config.height,
                    );
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                let forward = Vec3::new(self.cam_yaw.sin(), 0.0, -self.cam_yaw.cos());
                let right = Vec3::new(self.cam_yaw.cos(), 0.0, self.cam_yaw.sin());
                match code {
                    KeyCode::KeyW | KeyCode::ArrowUp => self.cam_pos += forward * 1.5,
                    KeyCode::KeyS | KeyCode::ArrowDown => self.cam_pos -= forward * 1.5,
                    KeyCode::KeyD => self.cam_pos += right * 1.5,
                    KeyCode::KeyA => self.cam_pos -= right * 1.5,
                    KeyCode::ArrowLeft => self.cam_yaw -= 0.1,
                    KeyCode::ArrowRight => self.cam_yaw += 0.1,
                    _ => {}
                }
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            WindowEvent::Touch(touch) => {
                use winit::event::TouchPhase;
                match touch.phase {
                    TouchPhase::Started => {
                        self.last_touch = Some((touch.location.x, touch.location.y));
                    }
                    TouchPhase::Moved => {
                        if let Some((lx, ly)) = self.last_touch {
                            let dx = (touch.location.x - lx) as f32;
                            let dy = (touch.location.y - ly) as f32;
                            self.cam_yaw += dx * 0.005;
                            let forward = Vec3::new(self.cam_yaw.sin(), 0.0, -self.cam_yaw.cos());
                            self.cam_pos -= forward * (dy * 0.08);
                        }
                        self.last_touch = Some((touch.location.x, touch.location.y));
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        self.last_touch = None;
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &self.gpu {
                    if let Ok(frame) = gpu.surface.get_current_texture() {
                        let color_view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        let aspect = gpu.config.width as f32 / gpu.config.height as f32;
                        let target = self.cam_pos
                            + Vec3::new(self.cam_yaw.sin(), -0.25, -self.cam_yaw.cos());
                        let cam = build_camera_matrix(self.cam_pos, target, aspect);

                        gpu.pipeline.render_parts(
                            &gpu.device,
                            &gpu.queue,
                            &color_view,
                            &gpu.depth_view,
                            &self.session.data_model.parts,
                            cam,
                        );
                        frame.present();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.session
            .physics
            .step(1.0 / 60.0, &mut self.session.data_model);
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }
}
