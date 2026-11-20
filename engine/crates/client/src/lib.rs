pub mod renderer;

use std::sync::Arc;

use glam::{Mat3, Vec3};
use renderer::{build_frame_uniform, HudState, PartPipeline};
use revival_core::{
    physics::PhysicsWorld, DataModel, DecalKind, PartMaterial, PartShape, SurfaceType, ToolIconKind,
};
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

pub struct CharacterState {
    spawn_pos: Vec3,
    pos: Vec3,
    vel: Vec3,
    yaw: f32,
    grounded: bool,
    walk_phase: f32,
    idle_time: f32,
    pub health: f32,
    part_indices: [usize; 6],
    tool_part_indices: [usize; 4],
}

impl CharacterState {
    pub fn spawn(dm: &mut DataModel) -> Self {
        let spawn_pos = dm
            .spawn_points
            .first()
            .copied()
            .unwrap_or(Vec3::new(0.0, 10.0, 0.0));
        // Classic Roblox Noob palette (Bright Yellow, Bright Blue, Br. Yellowish Green)
        let yellow = [0.96, 0.80, 0.19];
        let blue = [0.05, 0.41, 0.67];
        let green = [0.29, 0.59, 0.29];
        let ident = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        let head = dm.parts.len();
        dm.add_part_styled(
            "CharHead",
            spawn_pos + Vec3::new(0.0, 1.55, 0.0),
            Vec3::new(1.22, 1.18, 1.22),
            ident,
            yellow,
            0.0,
            true,
            PartShape::Head,
            PartMaterial::SmoothPlastic,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::ClassicFace,
        );
        let torso = dm.parts.len();
        dm.add_part_styled(
            "CharTorso",
            spawn_pos,
            Vec3::new(2.0, 2.0, 1.0),
            ident,
            blue,
            0.0,
            true,
            PartShape::BevelBlock,
            PartMaterial::SmoothPlastic,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::ClassicShirt,
        );
        let l_arm = dm.parts.len();
        dm.add_part_styled(
            "CharLeftArm",
            spawn_pos + Vec3::new(-1.5, 0.0, 0.0),
            Vec3::new(1.0, 2.0, 1.0),
            ident,
            yellow,
            0.0,
            true,
            PartShape::BevelBlock,
            PartMaterial::SmoothPlastic,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        let r_arm = dm.parts.len();
        dm.add_part_styled(
            "CharRightArm",
            spawn_pos + Vec3::new(1.5, 0.0, 0.0),
            Vec3::new(1.0, 2.0, 1.0),
            ident,
            yellow,
            0.0,
            true,
            PartShape::BevelBlock,
            PartMaterial::SmoothPlastic,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        let l_leg = dm.parts.len();
        dm.add_part_styled(
            "CharLeftLeg",
            spawn_pos + Vec3::new(-0.5, -2.0, 0.0),
            Vec3::new(1.0, 2.0, 1.0),
            ident,
            green,
            0.0,
            true,
            PartShape::BevelBlock,
            PartMaterial::SmoothPlastic,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::ClassicPants,
        );
        let r_leg = dm.parts.len();
        dm.add_part_styled(
            "CharRightLeg",
            spawn_pos + Vec3::new(0.5, -2.0, 0.0),
            Vec3::new(1.0, 2.0, 1.0),
            ident,
            green,
            0.0,
            true,
            PartShape::BevelBlock,
            PartMaterial::SmoothPlastic,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::ClassicPants,
        );
        // 4 reusable 3D sub-parts for multi-part equipped tools (hidden when unequipped)
        let mut tool_part_indices = [0usize; 4];
        for (i, slot) in tool_part_indices.iter_mut().enumerate() {
            *slot = dm.parts.len();
            dm.add_part_styled(
                format!("CharTool{i}"),
                spawn_pos,
                Vec3::splat(0.2),
                ident,
                [0.75, 0.80, 0.85],
                1.0,
                true,
                PartShape::Block,
                PartMaterial::Metal,
                SurfaceType::Smooth,
                SurfaceType::Smooth,
                DecalKind::None,
            );
        }

        Self {
            spawn_pos,
            pos: spawn_pos,
            vel: Vec3::ZERO,
            yaw: 0.0,
            grounded: false,
            walk_phase: 0.0,
            idle_time: 0.0,
            health: 100.0,
            part_indices: [head, torso, l_arm, r_arm, l_leg, r_leg],
            tool_part_indices,
        }
    }

    fn update_tool_parts(
        &self,
        base_pos: Vec3,
        body_rot: Mat3,
        active_tool: i32,
        dm: &mut DataModel,
    ) {
        // Hide all 4 tool sub-parts by default
        for &idx in &self.tool_part_indices {
            if let Some(tp) = dm.parts.get_mut(idx) {
                tp.transparency = 1.0;
            }
        }
        if active_tool < 0 || (active_tool as usize) >= dm.tools.len() {
            return;
        }
        let tool = dm.tools[active_tool as usize].clone();
        // Hand anchor at the tip of the forward-raised right arm
        let hand = Vec3::new(1.5, 0.8, -1.65);

        let set_sub = |dm: &mut DataModel,
                       slot: usize,
                       local_off: Vec3,
                       local_rot: Mat3,
                       size: Vec3,
                       color: [f32; 3],
                       shape: PartShape,
                       mat: PartMaterial| {
            if let Some(tp) = dm.parts.get_mut(self.tool_part_indices[slot]) {
                tp.position = base_pos + body_rot * (hand + local_off);
                tp.rotation = (body_rot * local_rot).to_cols_array_2d();
                tp.size = size;
                tp.color = color;
                tp.shape = shape;
                tp.material = mat;
                tp.transparency = 0.0;
            }
        };

        match tool.icon {
            ToolIconKind::Sword => {
                // Classic LinkedSword: black grip + gold crossguard + silver blade + pointed tip
                set_sub(dm, 0, Vec3::new(0.0, 0.0, -0.05), Mat3::IDENTITY, Vec3::new(0.22, 0.24, 0.85), [0.14, 0.14, 0.16], PartShape::Block, PartMaterial::SmoothPlastic);
                set_sub(dm, 1, Vec3::new(0.0, 0.0, -0.52), Mat3::IDENTITY, Vec3::new(0.95, 0.24, 0.20), [0.88, 0.70, 0.16], PartShape::Block, PartMaterial::Metal);
                set_sub(dm, 2, Vec3::new(0.0, 0.0, -2.05), Mat3::IDENTITY, Vec3::new(0.14, 0.30, 2.85), [0.78, 0.83, 0.88], PartShape::Block, PartMaterial::Metal);
                set_sub(dm, 3, Vec3::new(0.0, 0.0, -3.72), Mat3::from_rotation_y(std::f32::consts::FRAC_PI_2), Vec3::new(0.50, 0.30, 0.14), [0.82, 0.87, 0.92], PartShape::Wedge, PartMaterial::Metal);
            }
            ToolIconKind::Slingshot => {
                // Classic Wooden Y-Fork Slingshot + red elastic band
                let wood = [0.46, 0.27, 0.12];
                set_sub(dm, 0, Vec3::new(0.0, 0.15, -0.15), Mat3::IDENTITY, Vec3::new(0.24, 0.95, 0.24), wood, PartShape::Block, PartMaterial::Wood);
                set_sub(dm, 1, Vec3::new(-0.32, 0.85, -0.15), Mat3::from_rotation_z(0.42), Vec3::new(0.20, 0.85, 0.22), wood, PartShape::Block, PartMaterial::Wood);
                set_sub(dm, 2, Vec3::new(0.32, 0.85, -0.15), Mat3::from_rotation_z(-0.42), Vec3::new(0.20, 0.85, 0.22), wood, PartShape::Block, PartMaterial::Wood);
                set_sub(dm, 3, Vec3::new(0.0, 1.15, -0.15), Mat3::IDENTITY, Vec3::new(0.95, 0.10, 0.10), [0.80, 0.18, 0.14], PartShape::Block, PartMaterial::SmoothPlastic);
            }
            ToolIconKind::RocketLauncher => {
                // Classic Olive Rocket Launcher tube + grip + red warhead + sight
                let cyl_rot = Mat3::from_rotation_x(std::f32::consts::FRAC_PI_2);
                set_sub(dm, 0, Vec3::new(0.0, 0.35, -0.75), cyl_rot, Vec3::new(0.72, 3.2, 0.72), [0.25, 0.35, 0.19], PartShape::Cylinder, PartMaterial::Metal);
                set_sub(dm, 1, Vec3::new(0.0, -0.05, -0.15), Mat3::IDENTITY, Vec3::new(0.26, 0.70, 0.28), [0.18, 0.18, 0.20], PartShape::Block, PartMaterial::SmoothPlastic);
                set_sub(dm, 2, Vec3::new(0.0, 0.35, -2.42), Mat3::IDENTITY, Vec3::new(0.56, 0.56, 0.56), [0.86, 0.16, 0.12], PartShape::Ball, PartMaterial::SmoothPlastic);
                set_sub(dm, 3, Vec3::new(0.0, 0.76, -1.65), Mat3::IDENTITY, Vec3::new(0.14, 0.24, 0.22), [0.16, 0.16, 0.18], PartShape::Block, PartMaterial::Metal);
            }
            ToolIconKind::Trowel => {
                // Classic Builder's Trowel: wooden handle + angled neck + wide flat metal blade
                set_sub(dm, 0, Vec3::new(0.0, 0.0, -0.10), Mat3::IDENTITY, Vec3::new(0.24, 0.24, 0.90), [0.48, 0.28, 0.12], PartShape::Block, PartMaterial::Wood);
                set_sub(dm, 1, Vec3::new(0.0, 0.0, -0.72), Mat3::IDENTITY, Vec3::new(0.16, 0.16, 0.45), [0.66, 0.70, 0.75], PartShape::Block, PartMaterial::Metal);
                set_sub(dm, 2, Vec3::new(0.0, 0.0, -1.45), Mat3::IDENTITY, Vec3::new(0.85, 0.10, 1.15), [0.76, 0.80, 0.85], PartShape::Block, PartMaterial::Metal);
            }
            ToolIconKind::Bomb => {
                // Classic Black Timebomb: round sphere + metal cap + fuse + glowing neon spark
                set_sub(dm, 0, Vec3::new(0.0, 0.15, -0.45), Mat3::IDENTITY, Vec3::splat(1.25), [0.10, 0.10, 0.12], PartShape::Ball, PartMaterial::SmoothPlastic);
                set_sub(dm, 1, Vec3::new(0.0, 0.80, -0.45), Mat3::IDENTITY, Vec3::new(0.32, 0.22, 0.32), [0.45, 0.47, 0.50], PartShape::Cylinder, PartMaterial::Metal);
                set_sub(dm, 2, Vec3::new(0.0, 1.05, -0.45), Mat3::from_rotation_z(-0.35), Vec3::new(0.10, 0.45, 0.10), [0.58, 0.48, 0.34], PartShape::Block, PartMaterial::Fabric);
                set_sub(dm, 3, Vec3::new(0.12, 1.28, -0.45), Mat3::IDENTITY, Vec3::splat(0.26), [1.0, 0.58, 0.08], PartShape::Ball, PartMaterial::Neon);
            }
            ToolIconKind::Superball => {
                // Classic Glossy Bright Red Superball
                set_sub(dm, 0, Vec3::new(0.0, 0.15, -0.45), Mat3::IDENTITY, Vec3::splat(1.35), tool.handle_color, PartShape::Ball, PartMaterial::SmoothPlastic);
            }
            ToolIconKind::Generic => {
                let reach = (tool.handle_size.z * 0.45).max(0.4);
                set_sub(dm, 0, Vec3::new(0.0, 0.0, -reach), Mat3::IDENTITY, tool.handle_size, tool.handle_color, tool.handle_shape, PartMaterial::SmoothPlastic);
            }
        }
    }

    pub fn sync_equipped_tool(&self, active_tool: i32, dm: &mut DataModel) {
        let body_rot = Mat3::from_rotation_y(-self.yaw);
        let has_tool = active_tool >= 0 && (active_tool as usize) < dm.tools.len();
        if has_tool {
            // Raise right arm forward 90 degrees in classic Roblox tool-hold pose
            let arm_rot = Mat3::from_rotation_x(std::f32::consts::FRAC_PI_2);
            let world_arm_rot = body_rot * arm_rot;
            if let Some(arm) = dm.parts.get_mut(self.part_indices[3]) {
                arm.position = self.pos + body_rot * (Vec3::new(1.5, 0.8, 0.0) + arm_rot * Vec3::new(0.0, -0.8, 0.0));
                arm.rotation = world_arm_rot.to_cols_array_2d();
            }
        }
        self.update_tool_parts(self.pos, body_rot, active_tool, dm);
    }

    fn step(
        &mut self,
        dt: f32,
        move_input: [f32; 2],
        jump: bool,
        cam_yaw: f32,
        active_tool: i32,
        dm: &mut DataModel,
    ) {
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
        let pre_land_vy = self.vel.y;
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
            if pre_land_vy < -52.0 {
                let dmg = ((-pre_land_vy - 52.0) * 1.4).min(65.0);
                self.health = (self.health - dmg).max(0.0);
            }
            self.pos.y = gy + 3.0;
            self.vel.y = 0.0;
            self.grounded = true;
        }

        // Slow classic Roblox health regeneration
        if self.health > 0.0 && self.health < 100.0 {
            self.health = (self.health + dt * 1.5).min(100.0);
        }

        if self.pos.y < -150.0 || self.health <= 0.0 {
            self.pos = self.spawn_pos;
            self.vel = Vec3::ZERO;
            self.health = 100.0;
        }

        // Classic R6 joint-pivot animation (rotates limbs around shoulders & hips)
        self.idle_time += dt;
        let body_rot = Mat3::from_rotation_y(-self.yaw);
        let idle_sway = (self.idle_time * 2.2).sin() * 0.055;
        let limb_angle = if !self.grounded {
            0.0
        } else if moving {
            self.walk_phase.sin() * 0.65
        } else {
            idle_sway
        };
        let has_tool = active_tool >= 0 && (active_tool as usize) < dm.tools.len();
        let arm_l_angle = if !self.grounded { 2.4 } else { -limb_angle };
        let arm_r_angle = if has_tool {
            std::f32::consts::FRAC_PI_2
        } else if !self.grounded {
            2.4
        } else {
            limb_angle
        };
        let leg_l_angle = if !self.grounded {
            0.2
        } else if moving {
            limb_angle
        } else {
            0.0
        };
        let leg_r_angle = if !self.grounded {
            -0.2
        } else if moving {
            -limb_angle
        } else {
            0.0
        };

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

        self.update_tool_parts(base_pos, body_rot, active_tool, dm);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn now_secs() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

#[cfg(target_arch = "wasm32")]
fn now_secs() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now() / 1000.0)
        .unwrap_or(0.0)
}

pub struct ClientApp {
    session: ClientSession,
    character: CharacterState,
    gpu: Option<GpuState>,
    #[cfg(target_arch = "wasm32")]
    pending_gpu: std::rc::Rc<std::cell::RefCell<Option<GpuState>>>,
    cam_yaw: f32,
    cam_pitch: f32,
    keys: [bool; 4], // W, S, A, D
    active_tool: i32,
    jump_requested: bool,
    mouse_down: bool,
    last_cursor: Option<(f64, f64)>,
    jump_touch: Option<u64>,
    move_touch: Option<(u64, f64, f64, f64, f64)>,
    cam_touch: Option<(u64, f64, f64, f64)>,
    start_time_secs: f64,
    last_tick_secs: f64,
}

impl ClientApp {
    pub fn new(mut session: ClientSession) -> Self {
        let character = CharacterState::spawn(&mut session.data_model);
        let now = now_secs();
        Self {
            session,
            character,
            gpu: None,
            #[cfg(target_arch = "wasm32")]
            pending_gpu: std::rc::Rc::new(std::cell::RefCell::new(None)),
            cam_yaw: 0.0,
            cam_pitch: 0.28,
            keys: [false; 4],
            active_tool: 0,
            jump_requested: false,
            mouse_down: false,
            last_cursor: None,
            jump_touch: None,
            move_touch: None,
            cam_touch: None,
            start_time_secs: now,
            last_tick_secs: now,
        }
    }

    pub fn auto_frame_camera(&mut self) {
        self.cam_yaw = 0.0;
        self.cam_pitch = 0.28;
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = EventLoop::new()?;
        event_loop.run_app(&mut self)?;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn run_wasm(self) -> Result<(), wasm_bindgen::JsValue> {
        use winit::platform::web::EventLoopExtWebSys;
        console_error_panic_hook::set_once();
        let event_loop = EventLoop::new()
            .map_err(|e| wasm_bindgen::JsValue::from_str(&format!("event loop error: {e}")))?;
        event_loop.spawn_app(self);
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

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WASM_RUNNING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static WASM_PENDING_SESSION: std::cell::RefCell<Option<ClientSession>> = const { std::cell::RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn wasm_launch_or_swap(session: ClientSession) -> Result<String, wasm_bindgen::JsValue> {
    let summary = format!(
        "Loaded {} parts, {} tools",
        session.data_model.parts.len(),
        session.data_model.tools.len()
    );
    let already_running = WASM_RUNNING.with(|r| r.get());
    if already_running {
        WASM_PENDING_SESSION.with(|p| *p.borrow_mut() = Some(session));
        return Ok(summary);
    }
    WASM_RUNNING.with(|r| r.set(true));
    let mut client = ClientApp::new(session);
    client.auto_frame_camera();
    client.run_wasm()?;
    Ok(summary)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start_baseplate() -> Result<String, wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let mut session = ClientSession::offline_baseplate();
    let _ = session.data_model.run_luau_scripts_mut();
    session.physics = PhysicsWorld::from_data_model(&session.data_model);
    wasm_launch_or_swap(session)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start_place_bytes(bytes: &[u8]) -> Result<String, wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let mut session = ClientSession::from_place_bytes(bytes)
        .map_err(|e| wasm_bindgen::JsValue::from_str(&format!("failed to parse place: {e:#}")))?;
    let _ = session.data_model.run_luau_scripts_mut();
    session.physics = PhysicsWorld::from_data_model(&session.data_model);
    wasm_launch_or_swap(session)
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
            let pending = self.pending_gpu.clone();
            let win_clone = window.clone();
            let w = size.width.max(1);
            let h = size.height.max(1);
            wasm_bindgen_futures::spawn_local(async move {
                let log_err = |msg: &str| {
                    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(msg));
                };
                let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::GL | wgpu::Backends::BROWSER_WEBGPU,
                    ..Default::default()
                });
                let surface = match instance.create_surface(win_clone.clone()) {
                    Ok(s) => s,
                    Err(e) => {
                        log_err(&format!("[revival-wasm] create_surface failed: {e}"));
                        return;
                    }
                };
                let Some(adapter) = instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        compatible_surface: Some(&surface),
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        force_fallback_adapter: false,
                    })
                    .await
                else {
                    log_err("[revival-wasm] request_adapter returned None (WebGL2 unavailable)");
                    return;
                };
                let desc = wgpu::DeviceDescriptor {
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    ..Default::default()
                };
                let (device, queue) = match adapter.request_device(&desc, None).await {
                    Ok(dq) => dq,
                    Err(e) => {
                        log_err(&format!("[revival-wasm] request_device failed: {e}"));
                        return;
                    }
                };
                let caps = surface.get_capabilities(&adapter);
                let Some(format) = caps.formats.first().copied() else {
                    log_err("[revival-wasm] no supported surface formats");
                    return;
                };
                let Some(alpha_mode) = caps.alpha_modes.first().copied() else {
                    log_err("[revival-wasm] no supported alpha modes");
                    return;
                };
                let config = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format,
                    width: w,
                    height: h,
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode,
                    view_formats: vec![],
                    desired_maximum_frame_latency: 2,
                };
                surface.configure(&device, &config);
                let depth_view = PartPipeline::create_depth_view(&device, config.width, config.height);
                let pipeline = PartPipeline::new(&device, format);
                web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
                    "[revival-wasm] wgpu initialized ({w}x{h}, {format:?})"
                )));
                *pending.borrow_mut() = Some(GpuState {
                    window: win_clone.clone(),
                    surface,
                    device,
                    queue,
                    config,
                    depth_view,
                    pipeline,
                });
                win_clone.request_redraw();
            });
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.gpu = None;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        #[cfg(target_arch = "wasm32")]
        if self.gpu.is_none() {
            if let Some(g) = self.pending_gpu.borrow_mut().take() {
                self.gpu = Some(g);
            }
        }
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
            WindowEvent::MouseInput { state, .. } => {
                self.mouse_down = state == ElementState::Pressed;
                if !self.mouse_down {
                    self.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.mouse_down {
                    if let Some((lx, ly)) = self.last_cursor {
                        let dx = (position.x - lx) as f32;
                        let dy = (position.y - ly) as f32;
                        self.cam_yaw += dx * 0.006;
                        self.cam_pitch = (self.cam_pitch + dy * 0.0045).clamp(-0.25, 1.15);
                    }
                    self.last_cursor = Some((position.x, position.y));
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
                    KeyCode::Digit1 if pressed => self.active_tool = if self.active_tool == 0 { -1 } else { 0 },
                    KeyCode::Digit2 if pressed => self.active_tool = if self.active_tool == 1 { -1 } else { 1 },
                    KeyCode::Digit3 if pressed => self.active_tool = if self.active_tool == 2 { -1 } else { 2 },
                    KeyCode::Digit4 if pressed => self.active_tool = if self.active_tool == 3 { -1 } else { 3 },
                    KeyCode::Digit5 if pressed => self.active_tool = if self.active_tool == 4 { -1 } else { 4 },
                    KeyCode::Digit6 if pressed => self.active_tool = if self.active_tool == 5 { -1 } else { 5 },
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
                                now_secs(),
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

                        let mut tool_icons = [0u32; 6];
                        for (i, t) in self.session.data_model.tools.iter().take(6).enumerate() {
                            tool_icons[i] = t.icon as u32;
                        }
                        let hud = HudState {
                            show_touch_hud,
                            stick_center_uv: stick_uv,
                            stick_knob_norm: knob_norm,
                            stick_active,
                            jump_pressed: self.jump_touch.is_some(),
                            screen_width: w,
                            screen_height: h,
                            health: (self.character.health / 100.0).clamp(0.0, 1.0),
                            tool_count: self.session.data_model.tools.len().min(6) as u32,
                            active_tool: self.active_tool,
                            tool_icons,
                        };

                        let time_secs = (now_secs() - self.start_time_secs) as f32;
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
                            true,
                        );
                        frame.present();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        #[cfg(target_arch = "wasm32")]
        {
            if self.gpu.is_none() {
                if let Some(g) = self.pending_gpu.borrow_mut().take() {
                    self.gpu = Some(g);
                }
            }
            if let Some(mut new_session) = WASM_PENDING_SESSION.with(|p| p.borrow_mut().take()) {
                self.character = CharacterState::spawn(&mut new_session.data_model);
                self.session = new_session;
                self.active_tool = if self.session.data_model.tools.is_empty() { -1 } else { 0 };
                self.auto_frame_camera();
            }
        }
        let now = now_secs();
        let elapsed = now - self.last_tick_secs;
        if elapsed < 0.016 {
            return;
        }
        let dt = (elapsed as f32).min(1.0 / 30.0);
        self.last_tick_secs = now;

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
        self.character.step(
            dt,
            [move_x, move_z],
            jump,
            self.cam_yaw,
            self.active_tool,
            &mut self.session.data_model,
        );
        self.session
            .physics
            .step(dt, &mut self.session.data_model);

        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }
}
