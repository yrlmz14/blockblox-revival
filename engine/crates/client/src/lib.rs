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

struct CharacterState {
    spawn_pos: Vec3,
    pos: Vec3,
    vel: Vec3,
    yaw: f32,
    grounded: bool,
    walk_phase: f32,
    part_indices: [usize; 6],
}

impl CharacterState {
    fn spawn(dm: &mut DataModel) -> Self {
        let spawn_pos = dm
            .spawn_points
            .first()
            .copied()
            .unwrap_or(Vec3::new(0.0, 10.0, 0.0));
        let yellow = [0.96, 0.80, 0.19];
        let blue = [0.13, 0.45, 0.85];
        let green = [0.25, 0.65, 0.25];

        let head = dm.parts.len();
        dm.add_part("CharHead", spawn_pos + Vec3::new(0.0, 1.5, 0.0), Vec3::new(1.2, 1.0, 1.0), yellow, true);
        let torso = dm.parts.len();
        dm.add_part("CharTorso", spawn_pos, Vec3::new(2.0, 2.0, 1.0), blue, true);
        let l_arm = dm.parts.len();
        dm.add_part("CharLeftArm", spawn_pos + Vec3::new(-1.5, 0.0, 0.0), Vec3::new(1.0, 2.0, 1.0), yellow, true);
        let r_arm = dm.parts.len();
        dm.add_part("CharRightArm", spawn_pos + Vec3::new(1.5, 0.0, 0.0), Vec3::new(1.0, 2.0, 1.0), yellow, true);
        let l_leg = dm.parts.len();
        dm.add_part("CharLeftLeg", spawn_pos + Vec3::new(-0.5, -2.0, 0.0), Vec3::new(1.0, 2.0, 1.0), green, true);
        let r_leg = dm.parts.len();
        dm.add_part("CharRightLeg", spawn_pos + Vec3::new(0.5, -2.0, 0.0), Vec3::new(1.0, 2.0, 1.0), green, true);

        Self {
            spawn_pos,
            pos: spawn_pos,
            vel: Vec3::ZERO,
            yaw: 0.0,
            grounded: false,
            walk_phase: 0.0,
            part_indices: [head, torso, l_arm, r_arm, l_leg, r_leg],
        }
    }

    fn step(&mut self, dt: f32, move_input: [f32; 2], jump: bool, cam_yaw: f32, dm: &mut DataModel) {
        let forward = Vec3::new(cam_yaw.sin(), 0.0, -cam_yaw.cos());
        let right = Vec3::new(cam_yaw.cos(), 0.0, cam_yaw.sin());
        let wish = forward * move_input[1] + right * move_input[0];
        let speed = 20.0;
        if wish.length_squared() > 0.01 {
            let dir = wish.normalize();
            self.vel.x = dir.x * speed;
            self.vel.z = dir.z * speed;
            self.yaw = dir.x.atan2(-dir.z);
            self.walk_phase += dt * 10.0;
        } else {
            self.vel.x *= 0.7;
            self.vel.z *= 0.7;
        }

        if jump && self.grounded {
            self.vel.y = 24.0;
            self.grounded = false;
        }
        self.vel.y -= 52.0 * dt;

        let prev_feet = self.pos.y - 3.0;
        self.pos += self.vel * dt;
        self.grounded = false;

        // Ground & platform collision against static world parts
        let feet_y = self.pos.y - 3.0;
        let char_r = 0.9;
        let char_count = self.part_indices[0];
        let mut best_y: Option<f32> = None;
        for p in &dm.parts[..char_count] {
            if p.transparency >= 0.95 {
                continue;
            }
            let hx = p.size.x * 0.5 + char_r;
            let hz = p.size.z * 0.5 + char_r;
            if (self.pos.x - p.position.x).abs() <= hx && (self.pos.z - p.position.z).abs() <= hz {
                let top_y = p.position.y + p.size.y * 0.5;
                if self.vel.y <= 0.0 && prev_feet >= top_y - 1.2 && feet_y <= top_y {
                    best_y = Some(best_y.map_or(top_y, |b: f32| b.max(top_y)));
                }
            }
        }
        if let Some(gy) = best_y {
            self.pos.y = gy + 3.0;
            self.vel.y = 0.0;
            self.grounded = true;
        }

        if self.pos.y < -150.0 {
            self.pos = self.spawn_pos;
            self.vel = Vec3::ZERO;
        }

        // Update 6 R6 character parts
        let rot = glam::Mat3::from_rotation_y(-self.yaw);
        let swing = if wish.length_squared() > 0.01 && self.grounded {
            self.walk_phase.sin() * 0.35
        } else {
            0.0
        };
        let offsets = [
            Vec3::new(0.0, 1.5, 0.0),
            Vec3::ZERO,
            Vec3::new(-1.5, 0.0, -swing),
            Vec3::new(1.5, 0.0, swing),
            Vec3::new(-0.5, -2.0, swing),
            Vec3::new(0.5, -2.0, -swing),
        ];
        let rot_arr = rot.to_cols_array_2d();
        for (i, &idx) in self.part_indices.iter().enumerate() {
            if let Some(p) = dm.parts.get_mut(idx) {
                p.position = self.pos + rot * offsets[i];
                p.rotation = rot_arr;
            }
        }
    }
}

pub struct ClientApp {
    session: ClientSession,
    character: CharacterState,
    gpu: Option<GpuState>,
    cam_yaw: f32,
    cam_pitch: f32,
    keys: [bool; 4], // W, S, A, D
    jump_requested: bool,
    move_touch: Option<(u64, f64, f64, f64, f64)>,
    cam_touch: Option<(u64, f64, f64, std::time::Instant)>,
    last_tick: std::time::Instant,
}

impl ClientApp {
    pub fn new(mut session: ClientSession) -> Self {
        let character = CharacterState::spawn(&mut session.data_model);
        Self {
            session,
            character,
            gpu: None,
            cam_yaw: 0.0,
            cam_pitch: 0.25,
            keys: [false; 4],
            jump_requested: false,
            move_touch: None,
            cam_touch: None,
            last_tick: std::time::Instant::now(),
        }
    }

    pub fn auto_frame_camera(&mut self) {
        self.cam_yaw = 0.0;
        self.cam_pitch = 0.25;
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
                        state,
                        ..
                    },
                ..
            } => {
                let pressed = state == ElementState::Pressed;
                match code {
                    KeyCode::KeyW | KeyCode::ArrowUp => self.keys[0] = pressed,
                    KeyCode::KeyS | KeyCode::ArrowDown => self.keys[1] = pressed,
                    KeyCode::KeyA => self.keys[2] = pressed,
                    KeyCode::KeyD => self.keys[3] = pressed,
                    KeyCode::Space if pressed => self.jump_requested = true,
                    KeyCode::ArrowLeft | KeyCode::KeyQ if pressed => self.cam_yaw -= 0.12,
                    KeyCode::ArrowRight | KeyCode::KeyE if pressed => self.cam_yaw += 0.12,
                    _ => {}
                }
            }
            WindowEvent::Touch(touch) => {
                use winit::event::TouchPhase;
                let width = self.gpu.as_ref().map_or(1080.0, |g| g.config.width as f64);
                let is_left = touch.location.x < width * 0.5;
                match touch.phase {
                    TouchPhase::Started => {
                        if is_left && self.move_touch.is_none() {
                            self.move_touch = Some((
                                touch.id,
                                touch.location.x,
                                touch.location.y,
                                touch.location.x,
                                touch.location.y,
                            ));
                        } else if !is_left && self.cam_touch.is_none() {
                            self.cam_touch = Some((
                                touch.id,
                                touch.location.x,
                                touch.location.y,
                                std::time::Instant::now(),
                            ));
                        }
                    }
                    TouchPhase::Moved => {
                        if let Some((id, sx, sy, _, _)) = self.move_touch {
                            if id == touch.id {
                                self.move_touch = Some((id, sx, sy, touch.location.x, touch.location.y));
                            }
                        }
                        if let Some((id, lx, ly, t0)) = self.cam_touch {
                            if id == touch.id {
                                let dx = (touch.location.x - lx) as f32;
                                let dy = (touch.location.y - ly) as f32;
                                self.cam_yaw += dx * 0.006;
                                self.cam_pitch = (self.cam_pitch + dy * 0.004).clamp(-0.2, 1.1);
                                self.cam_touch = Some((id, touch.location.x, touch.location.y, t0));
                            }
                        }
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        if self.move_touch.is_some_and(|(id, ..)| id == touch.id) {
                            self.move_touch = None;
                        }
                        if let Some((id, _, _, t0)) = self.cam_touch {
                            if id == touch.id {
                                if t0.elapsed().as_millis() < 220 {
                                    self.jump_requested = true;
                                }
                                self.cam_touch = None;
                            }
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &mut self.gpu {
                    if let Ok(frame) = gpu.surface.get_current_texture() {
                        let color_view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        let aspect = gpu.config.width as f32 / gpu.config.height as f32;
                        let dist = 14.0;
                        let cam_pos = self.character.pos
                            + Vec3::new(
                                -self.cam_yaw.sin() * dist * self.cam_pitch.cos(),
                                3.0 + dist * self.cam_pitch.sin(),
                                self.cam_yaw.cos() * dist * self.cam_pitch.cos(),
                            );
                        let target = self.character.pos + Vec3::new(0.0, 1.5, 0.0);
                        let cam = build_camera_matrix(cam_pos, target, aspect);

                        gpu.pipeline.render_parts(
                            &gpu.device,
                            &gpu.queue,
                            &color_view,
                            &gpu.depth_view,
                            &self.session.data_model.parts,
                            self.session.data_model.sky.sky_top,
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
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_tick);
        if elapsed < std::time::Duration::from_millis(16) {
            return;
        }
        let dt = elapsed.as_secs_f32().min(1.0 / 30.0);
        self.last_tick = now;

        let mut move_x = (self.keys[3] as i32 - self.keys[2] as i32) as f32;
        let mut move_z = (self.keys[0] as i32 - self.keys[1] as i32) as f32;
        if let Some((_, sx, sy, cx, cy)) = self.move_touch {
            let dx = ((cx - sx) / 80.0).clamp(-1.0, 1.0) as f32;
            let dy = ((sy - cy) / 80.0).clamp(-1.0, 1.0) as f32;
            if dx.abs() > 0.1 || dy.abs() > 0.1 {
                move_x = dx;
                move_z = dy;
            }
        }

        let jump = std::mem::take(&mut self.jump_requested);
        self.character
            .step(dt, [move_x, move_z], jump, self.cam_yaw, &mut self.session.data_model);
        self.session
            .physics
            .step(dt, &mut self.session.data_model);

        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }
}
