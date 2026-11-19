pub mod renderer;

use std::sync::Arc;

use glam::{Mat3, Vec3};
use renderer::{build_frame_uniform, HudState, PartPipeline};
use revival_core::{physics::PhysicsWorld, DataModel, PartShape};
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
        let blue = [0.05, 0.41, 0.67];
        let green = [0.29, 0.59, 0.29];
        let ident = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        let head = dm.parts.len();
        dm.add_part_shaped(
            "CharHead",
            spawn_pos + Vec3::new(0.0, 1.5, 0.0),
            Vec3::new(1.2, 1.15, 1.2),
            ident,
            yellow,
            0.0,
            true,
            PartShape::Cylinder,
        );
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
        let speed = 18.0;
        let moving = wish.length_squared() > 0.01;
        if moving {
            let mag = wish.length().min(1.0);
            let dir = wish.normalize();
            self.vel.x = dir.x * speed * mag;
            self.vel.z = dir.z * speed * mag;
            let target_yaw = dir.x.atan2(-dir.z);
            let mut diff = target_yaw - self.yaw;
            while diff > std::f32::consts::PI {
                diff -= std::f32::consts::TAU;
            }
            while diff < -std::f32::consts::PI {
                diff += std::f32::consts::TAU;
            }
            self.yaw += diff * (dt * 14.0).min(1.0);
            self.walk_phase += dt * 11.0 * mag;
        } else {
            self.vel.x *= 0.68;
            self.vel.z *= 0.68;
        }

        if jump && self.grounded {
            self.vel.y = 26.0;
            self.grounded = false;
        }
        self.vel.y -= 58.0 * dt;

        let prev_feet = self.pos.y - 3.0;
        self.pos += self.vel * dt;
        self.grounded = false;

        // Ground, step-up & wedge-ramp collision against static world parts
        let feet_y = self.pos.y - 3.0;
        let char_r = 0.85;
        let char_count = self.part_indices[0];
        let mut best_y: Option<f32> = None;
        for p in &dm.parts[..char_count] {
            if p.transparency >= 0.95 {
                continue;
            }
            let hx = p.size.x * 0.5 + char_r;
            let hz = p.size.z * 0.5 + char_r;
            let dx = self.pos.x - p.position.x;
            let dz = self.pos.z - p.position.z;
            if dx.abs() <= hx && dz.abs() <= hz {
                let top_y = if p.shape == PartShape::Wedge {
                    // Slope rises from +Z (bottom) to -Z (top) in local space
                    let t = (0.5 - (dz / p.size.z.max(0.1))).clamp(0.0, 1.0);
                    p.position.y - p.size.y * 0.5 + t * p.size.y
                } else {
                    p.position.y + p.size.y * 0.5
                };
                if self.vel.y <= 1.0 && prev_feet >= top_y - 1.8 && feet_y <= top_y + 0.35 {
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

        // Classic R6 joint-pivot animation (rotates limbs around shoulders & hips)
        let body_rot = Mat3::from_rotation_y(-self.yaw);
        let limb_angle = if !self.grounded {
            0.0
        } else if moving {
            self.walk_phase.sin() * 0.65
        } else {
            0.0
        };
        let arm_l_angle = if !self.grounded { 2.4 } else { -limb_angle };
        let arm_r_angle = if !self.grounded { 2.4 } else { limb_angle };
        let leg_l_angle = if !self.grounded { 0.2 } else { limb_angle };
        let leg_r_angle = if !self.grounded { -0.2 } else { -limb_angle };

        let bob = if moving && self.grounded {
            (self.walk_phase * 2.0).sin().abs() * 0.12
        } else {
            0.0
        };
        let base_pos = self.pos + Vec3::new(0.0, bob, 0.0);

        let set_limb = |dm: &mut DataModel, idx: usize, pivot: Vec3, local_from_pivot: Vec3, pitch: f32| {
            let local_rot = Mat3::from_rotation_x(pitch);
            let world_rot = body_rot * local_rot;
            if let Some(p) = dm.parts.get_mut(idx) {
                p.position = base_pos + body_rot * (pivot + local_rot * local_from_pivot);
                p.rotation = world_rot.to_cols_array_2d();
            }
        };

        // Head & Torso
        set_limb(dm, self.part_indices[0], Vec3::new(0.0, 1.55, 0.0), Vec3::ZERO, 0.0);
        set_limb(dm, self.part_indices[1], Vec3::ZERO, Vec3::ZERO, 0.0);
        // Left & Right Arms (pivot at shoulder y=+0.8)
        set_limb(dm, self.part_indices[2], Vec3::new(-1.5, 0.8, 0.0), Vec3::new(0.0, -0.8, 0.0), arm_l_angle);
        set_limb(dm, self.part_indices[3], Vec3::new(1.5, 0.8, 0.0), Vec3::new(0.0, -0.8, 0.0), arm_r_angle);
        // Left & Right Legs (pivot at hip y=-1.0)
        set_limb(dm, self.part_indices[4], Vec3::new(-0.5, -1.0, 0.0), Vec3::new(0.0, -1.0, 0.0), leg_l_angle);
        set_limb(dm, self.part_indices[5], Vec3::new(0.5, -1.0, 0.0), Vec3::new(0.0, -1.0, 0.0), leg_r_angle);
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
    jump_touch: Option<u64>,
    move_touch: Option<(u64, f64, f64, f64, f64)>,
    cam_touch: Option<(u64, f64, f64, std::time::Instant)>,
    start_time: std::time::Instant,
    last_tick: std::time::Instant,
}

impl ClientApp {
    pub fn new(mut session: ClientSession) -> Self {
        let character = CharacterState::spawn(&mut session.data_model);
        let now = std::time::Instant::now();
        Self {
            session,
            character,
            gpu: None,
            cam_yaw: 0.0,
            cam_pitch: 0.28,
            keys: [false; 4],
            jump_requested: false,
            jump_touch: None,
            move_touch: None,
            cam_touch: None,
            start_time: now,
            last_tick: now,
        }
    }

    pub fn auto_frame_camera(&mut self) {
        self.cam_yaw = 0.0;
        self.cam_pitch = 0.28;
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
                let (width, height) = self
                    .gpu
                    .as_ref()
                    .map_or((1080.0, 2340.0), |g| (g.config.width as f64, g.config.height as f64));
                let short_side = width.min(height);
                let jump_r = short_side * 0.11; // slightly generous hit target
                let jump_cx = width - short_side * 0.15;
                let jump_cy = height - short_side * 0.16;
                let dx_j = touch.location.x - jump_cx;
                let dy_j = touch.location.y - jump_cy;
                let in_jump_btn = (dx_j * dx_j + dy_j * dy_j) <= (jump_r * jump_r);
                let is_left = touch.location.x < width * 0.5;

                match touch.phase {
                    TouchPhase::Started => {
                        if in_jump_btn {
                            self.jump_touch = Some(touch.id);
                            self.jump_requested = true;
                        } else if is_left && self.move_touch.is_none() {
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
                                self.cam_yaw += dx * 0.0055;
                                self.cam_pitch = (self.cam_pitch + dy * 0.004).clamp(-0.25, 1.15);
                                self.cam_touch = Some((id, touch.location.x, touch.location.y, t0));
                            }
                        }
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        if self.jump_touch == Some(touch.id) {
                            self.jump_touch = None;
                        }
                        if self.move_touch.is_some_and(|(id, ..)| id == touch.id) {
                            self.move_touch = None;
                        }
                        if let Some((id, _, _, _)) = self.cam_touch {
                            if id == touch.id {
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
                        let w = gpu.config.width as f32;
                        let h = gpu.config.height as f32;
                        let aspect = w / h.max(1.0);
                        let dist = 15.0;
                        let cam_pos = self.character.pos
                            + Vec3::new(
                                -self.cam_yaw.sin() * dist * self.cam_pitch.cos(),
                                3.2 + dist * self.cam_pitch.sin(),
                                self.cam_yaw.cos() * dist * self.cam_pitch.cos(),
                            );
                        let target = self.character.pos + Vec3::new(0.0, 1.5, 0.0);

                        let short_side = w.min(h);
                        let stick_r = (short_side * 0.115).max(1.0);
                        let default_stick_uv = [(short_side * 0.18) / w.max(1.0), 1.0 - (short_side * 0.18) / h.max(1.0)];
                        let (stick_uv, knob_norm, stick_active) =
                            if let Some((_, sx, sy, cx, cy)) = self.move_touch {
                                let dx = (cx - sx) as f32;
                                let dy = (cy - sy) as f32;
                                let len = (dx * dx + dy * dy).sqrt();
                                let scale = if len > stick_r { stick_r / len } else { 1.0 };
                                (
                                    [(sx as f32 / w.max(1.0)).clamp(0.05, 0.45), (sy as f32 / h.max(1.0)).clamp(0.2, 0.95)],
                                    [(dx * scale) / stick_r, (dy * scale) / stick_r],
                                    true,
                                )
                            } else {
                                (default_stick_uv, [0.0, 0.0], false)
                            };

                        let show_touch_hud = cfg!(target_os = "android")
                            || self.move_touch.is_some()
                            || self.cam_touch.is_some()
                            || self.jump_touch.is_some();

                        let hud = HudState {
                            show_touch_hud,
                            stick_center_uv: stick_uv,
                            stick_knob_norm: knob_norm,
                            stick_active,
                            jump_pressed: self.jump_touch.is_some(),
                            screen_width: w,
                            screen_height: h,
                        };

                        let time_secs = self.start_time.elapsed().as_secs_f32();
                        let uniform = build_frame_uniform(
                            cam_pos,
                            target,
                            aspect,
                            &self.session.data_model.sky,
                            time_secs,
                            hud,
                        );

                        gpu.pipeline.render_frame(
                            &gpu.device,
                            &gpu.queue,
                            &color_view,
                            &gpu.depth_view,
                            &self.session.data_model.parts,
                            uniform,
                            show_touch_hud,
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
            let short_side = self
                .gpu
                .as_ref()
                .map_or(1080.0, |g| (g.config.width.min(g.config.height)) as f64);
            let stick_r = (short_side * 0.115).max(40.0);
            let dx = ((cx - sx) / stick_r).clamp(-1.0, 1.0) as f32;
            let dy = ((sy - cy) / stick_r).clamp(-1.0, 1.0) as f32;
            if dx.abs() > 0.08 || dy.abs() > 0.08 {
                move_x = dx;
                move_z = dy;
            }
        }

        let jump = std::mem::take(&mut self.jump_requested) || self.jump_touch.is_some();
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
