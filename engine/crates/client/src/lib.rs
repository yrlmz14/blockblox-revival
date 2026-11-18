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
}

impl ClientApp {
    pub fn new(session: ClientSession) -> Self {
        Self {
            session,
            gpu: None,
            cam_pos: Vec3::new(0.0, 10.0, 24.0),
            cam_yaw: 0.0,
        }
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = EventLoop::new()?;
        event_loop.run_app(&mut self)?;
        Ok(())
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_org_libreblox_app_RevivalBridge_nativeStartOfflineBytes(
    _env: *mut std::ffi::c_void,
    _class: *mut std::ffi::c_void,
    _bytes: *mut std::ffi::c_void,
) -> bool {
    true
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_org_libreblox_app_RevivalBridge_nativeStartOnlineWs(
    _env: *mut std::ffi::c_void,
    _class: *mut std::ffi::c_void,
    _ws_url: *mut std::ffi::c_void,
) -> bool {
    true
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

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let size = window.inner_size();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let instance = wgpu::Instance::default();
            let surface = instance
                .create_surface(window.clone())
                .expect("create surface");
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..Default::default()
                }))
                .expect("find GPU adapter");

            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                    .expect("create GPU device");

            let caps = surface.get_capabilities(&adapter);
            let format = caps.formats[0];
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            let depth_view = PartPipeline::create_depth_view(&device, config.width, config.height);
            let pipeline = PartPipeline::new(&device, format);

            self.gpu = Some(GpuState {
                window,
                surface,
                device,
                queue,
                config,
                depth_view,
                pipeline,
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = size;
            // On wasm32, surface/adapter/device are initialized asynchronously via wasm_bindgen_futures::spawn_local.
            let _ = window;
        }
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
