#[cfg(feature = "luau")]
pub mod luau_api;
pub mod physics;

use std::io::Cursor;

use anyhow::{Context, Result};
use glam::{Mat4, Vec3};
use rbx_dom_weak::{types::Variant, ustr, Instance, WeakDom};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PartShape {
    #[default]
    Block,
    Wedge,
    Ball,
    Cylinder,
    Head,
    BevelBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u32)]
pub enum PartMaterial {
    #[default]
    Plastic = 0,
    SmoothPlastic = 1,
    Wood = 2,
    WoodPlanks = 3,
    Brick = 4,
    Concrete = 5,
    Slate = 6,
    Cobblestone = 7,
    Metal = 8,
    DiamondPlate = 9,
    Grass = 10,
    Sand = 11,
    Fabric = 12,
    Ice = 13,
    Glass = 14,
    Neon = 15,
}

impl PartMaterial {
    pub fn from_roblox_enum(val: u32) -> Self {
        match val {
            256 => Self::Plastic,
            272 => Self::SmoothPlastic,
            288 => Self::Neon,
            512 => Self::Wood,
            528 => Self::WoodPlanks,
            784 | 800 | 832 => Self::Slate,
            816 => Self::Concrete,
            848 => Self::Brick,
            864 | 880 => Self::Cobblestone,
            1040 | 1072 | 1088 => Self::Metal,
            1056 => Self::DiamondPlate,
            1280 | 1284 => Self::Grass,
            1296 => Self::Sand,
            1312 => Self::Fabric,
            1536 => Self::Ice,
            1568 => Self::Glass,
            _ => Self::Plastic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u32)]
pub enum SurfaceType {
    Smooth = 0,
    #[default]
    Studs = 1,
    Inlet = 2,
    Weld = 3,
    Universal = 4,
}

impl SurfaceType {
    pub fn from_roblox_enum(val: u32) -> Self {
        match val {
            0 | 10 => Self::Smooth,
            2 | 1 => Self::Weld,
            3 => Self::Studs,
            4 => Self::Inlet,
            5 => Self::Universal,
            _ => Self::Smooth,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u32)]
pub enum DecalKind {
    #[default]
    None = 0,
    ClassicFace = 1,
    Spawn = 2,
    ClassicShirt = 3,
    ClassicPants = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    pub id: u64,
    pub name: String,
    pub position: Vec3,
    pub size: Vec3,
    pub rotation: [[f32; 3]; 3],
    pub color: [f32; 3],
    pub transparency: f32,
    pub anchored: bool,
    #[serde(default)]
    pub shape: PartShape,
    #[serde(default)]
    pub material: PartMaterial,
    #[serde(default)]
    pub top_surface: SurfaceType,
    #[serde(default = "default_bottom_surface")]
    pub bottom_surface: SurfaceType,
    #[serde(default)]
    pub decal: DecalKind,
}

fn default_bottom_surface() -> SurfaceType {
    SurfaceType::Inlet
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkyConfig {
    pub sky_top: [f32; 3],
    pub sky_bottom: [f32; 3],
    pub sun_direction: Vec3,
    pub ambient: [f32; 3],
    pub brightness: f32,
    #[serde(default = "default_fog_start")]
    pub fog_start: f32,
    #[serde(default = "default_fog_end")]
    pub fog_end: f32,
    pub skybox_bk: Option<String>,
    pub skybox_dn: Option<String>,
    pub skybox_ft: Option<String>,
    pub skybox_lf: Option<String>,
    pub skybox_rt: Option<String>,
    pub skybox_up: Option<String>,
    pub celestial_bodies_shown: bool,
}

fn default_fog_start() -> f32 {
    420.0
}

fn default_fog_end() -> f32 {
    1400.0
}

impl Default for SkyConfig {
    fn default() -> Self {
        Self {
            sky_top: [0.27, 0.56, 0.92],
            sky_bottom: [0.74, 0.89, 0.99],
            sun_direction: Vec3::new(0.42, 0.85, 0.32).normalize(),
            ambient: [0.38, 0.42, 0.48],
            brightness: 1.15,
            fog_start: default_fog_start(),
            fog_end: default_fog_end(),
            skybox_bk: None,
            skybox_dn: None,
            skybox_ft: None,
            skybox_lf: None,
            skybox_rt: None,
            skybox_up: None,
            celestial_bodies_shown: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstanceNode {
    pub name: String,
    pub class_name: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub part_index: Option<usize>,
    pub script_index: Option<usize>,
    pub disabled: bool,
    pub value_num: f64,
    pub value_str: String,
    pub value_bool: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u32)]
pub enum ToolIconKind {
    #[default]
    Generic = 0,
    Sword = 1,
    Slingshot = 2,
    RocketLauncher = 3,
    Trowel = 4,
    Bomb = 5,
    Superball = 6,
}

impl ToolIconKind {
    pub fn from_name_and_texture(
        name: &str,
        texture_id: Option<&str>,
        mesh_id: Option<&str>,
    ) -> Self {
        let hay = format!(
            "{} {} {}",
            name.to_ascii_lowercase(),
            texture_id.unwrap_or("").to_ascii_lowercase(),
            mesh_id.unwrap_or("").to_ascii_lowercase()
        );
        if hay.contains("sword") || hay.contains("blade") {
            Self::Sword
        } else if hay.contains("sling") {
            Self::Slingshot
        } else if hay.contains("rocket") || hay.contains("launcher") {
            Self::RocketLauncher
        } else if hay.contains("trowel") || hay.contains("wall") {
            Self::Trowel
        } else if hay.contains("bomb") {
            Self::Bomb
        } else if hay.contains("superball") || hay.contains("ball") {
            Self::Superball
        } else {
            Self::Generic
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolItem {
    pub name: String,
    pub icon: ToolIconKind,
    pub texture_id: Option<String>,
    pub mesh_id: Option<String>,
    pub grip_pos: Vec3,
    pub handle_color: [f32; 3],
    pub handle_size: Vec3,
    pub handle_shape: PartShape,
}

#[derive(Debug, Clone)]
pub struct DataModel {
    pub parts: Vec<Part>,
    pub spawn_points: Vec<Vec3>,
    pub scripts: Vec<(String, String)>,
    pub script_instances: Vec<usize>,
    pub instances: Vec<InstanceNode>,
    pub sky: SkyConfig,
    pub tools: Vec<ToolItem>,
    next_id: u64,
}

impl Default for DataModel {
    fn default() -> Self {
        Self {
            parts: Vec::new(),
            spawn_points: Vec::new(),
            scripts: Vec::new(),
            script_instances: Vec::new(),
            instances: vec![InstanceNode {
                name: "DataModel".into(),
                class_name: "DataModel".into(),
                parent: None,
                children: Vec::new(),
                part_index: None,
                script_index: None,
                disabled: false,
                value_num: 0.0,
                value_str: String::new(),
                value_bool: false,
            }],
            sky: SkyConfig::default(),
            tools: Vec::new(),
            next_id: 0,
        }
    }
}

impl DataModel {
    pub fn ensure_service(&mut self, class_name: &str) -> usize {
        if let Some(idx) = self.instances[0]
            .children
            .iter()
            .copied()
            .find(|&i| self.instances[i].class_name == class_name || self.instances[i].name == class_name)
        {
            return idx;
        }
        let idx = self.instances.len();
        self.instances.push(InstanceNode {
            name: class_name.to_string(),
            class_name: class_name.to_string(),
            parent: Some(0),
            children: Vec::new(),
            part_index: None,
            script_index: None,
            disabled: false,
            value_num: 0.0,
            value_str: String::new(),
            value_bool: false,
        });
        self.instances[0].children.push(idx);
        idx
    }

    pub fn new_baseplate() -> Self {
        let mut dm = Self::default();
        let ws = dm.ensure_service("Workspace");
        let ident = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        // Classic Roblox Baseplate (medium stone grey with studs)
        let base_id = dm.add_part_styled(
            "Baseplate",
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(200.0, 2.0, 200.0),
            ident,
            [0.39, 0.41, 0.44],
            0.0,
            true,
            PartShape::Block,
            PartMaterial::Plastic,
            SurfaceType::Studs,
            SurfaceType::Inlet,
            DecalKind::None,
        );
        dm.attach_part_instance("Baseplate", ws, (base_id - 1) as usize);

        // Classic SpawnLocation with Roblox sunburst spawn decal
        let spawn_id = dm.add_part_styled(
            "SpawnLocation",
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::new(6.0, 1.0, 6.0),
            ident,
            [0.64, 0.64, 0.65],
            0.0,
            true,
            PartShape::Block,
            PartMaterial::Plastic,
            SurfaceType::Studs,
            SurfaceType::Inlet,
            DecalKind::Spawn,
        );
        dm.attach_part_instance("SpawnLocation", ws, (spawn_id - 1) as usize);
        dm.spawn_points.push(Vec3::new(0.0, 4.0, 0.0));

        // Classic red plastic ramp + brushed metal platform
        let ramp_id = dm.add_part_styled(
            "ClassicRamp",
            Vec3::new(14.0, 3.0, -10.0),
            Vec3::new(8.0, 6.0, 12.0),
            ident,
            [0.77, 0.16, 0.11],
            0.0,
            true,
            PartShape::Wedge,
            PartMaterial::Plastic,
            SurfaceType::Studs,
            SurfaceType::Inlet,
            DecalKind::None,
        );
        dm.attach_part_instance("ClassicRamp", ws, (ramp_id - 1) as usize);

        let tower_id = dm.add_part_styled(
            "ClassicPlatform",
            Vec3::new(14.0, 5.5, -22.0),
            Vec3::new(12.0, 1.0, 12.0),
            ident,
            [0.15, 0.45, 0.78],
            0.0,
            true,
            PartShape::Block,
            PartMaterial::DiamondPlate,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        dm.attach_part_instance("ClassicPlatform", ws, (tower_id - 1) as usize);

        // Classic Happy Home in Robloxia style mini brick cottage
        let wall_back = dm.add_part_styled(
            "BrickWallBack",
            Vec3::new(-14.0, 4.0, -20.0),
            Vec3::new(12.0, 8.0, 1.5),
            ident,
            [0.68, 0.25, 0.20],
            0.0,
            true,
            PartShape::Block,
            PartMaterial::Brick,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        dm.attach_part_instance("BrickWallBack", ws, (wall_back - 1) as usize);

        let wall_left = dm.add_part_styled(
            "BrickWallLeft",
            Vec3::new(-19.25, 4.0, -14.75),
            Vec3::new(1.5, 8.0, 9.0),
            ident,
            [0.68, 0.25, 0.20],
            0.0,
            true,
            PartShape::Block,
            PartMaterial::Brick,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        dm.attach_part_instance("BrickWallLeft", ws, (wall_left - 1) as usize);

        let wall_right = dm.add_part_styled(
            "BrickWallRight",
            Vec3::new(-8.75, 4.0, -14.75),
            Vec3::new(1.5, 8.0, 9.0),
            ident,
            [0.68, 0.25, 0.20],
            0.0,
            true,
            PartShape::Block,
            PartMaterial::Brick,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        dm.attach_part_instance("BrickWallRight", ws, (wall_right - 1) as usize);

        let roof = dm.add_part_styled(
            "WoodRoof",
            Vec3::new(-14.0, 8.5, -15.0),
            Vec3::new(13.5, 1.0, 12.0),
            ident,
            [0.48, 0.30, 0.16],
            0.0,
            true,
            PartShape::Block,
            PartMaterial::WoodPlanks,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        dm.attach_part_instance("WoodRoof", ws, (roof - 1) as usize);

        let window = dm.add_part_styled(
            "GlassWindow",
            Vec3::new(-14.0, 4.2, -10.3),
            Vec3::new(6.0, 4.0, 0.4),
            ident,
            [0.58, 0.85, 0.98],
            0.55,
            true,
            PartShape::Block,
            PartMaterial::Glass,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        dm.attach_part_instance("GlassWindow", ws, (window - 1) as usize);

        // Classic Roblox blocky tree (Grass lawn + Wood trunk + Grass leaves)
        let lawn = dm.add_part_styled(
            "GrassLawn",
            Vec3::new(-14.0, 0.15, -2.0),
            Vec3::new(14.0, 0.3, 12.0),
            ident,
            [0.24, 0.56, 0.22],
            0.0,
            true,
            PartShape::Block,
            PartMaterial::Grass,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        dm.attach_part_instance("GrassLawn", ws, (lawn - 1) as usize);

        let trunk = dm.add_part_styled(
            "TreeTrunk",
            Vec3::new(-17.0, 3.5, -2.0),
            Vec3::new(2.0, 7.0, 2.0),
            ident,
            [0.42, 0.26, 0.14],
            0.0,
            true,
            PartShape::Block,
            PartMaterial::Wood,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        dm.attach_part_instance("TreeTrunk", ws, (trunk - 1) as usize);

        let leaves = dm.add_part_styled(
            "TreeCanopy",
            Vec3::new(-17.0, 8.5, -2.0),
            Vec3::new(7.0, 5.0, 7.0),
            ident,
            [0.18, 0.52, 0.18],
            0.0,
            true,
            PartShape::Block,
            PartMaterial::Grass,
            SurfaceType::Studs,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        dm.attach_part_instance("TreeCanopy", ws, (leaves - 1) as usize);

        // Glowing Neon beacon on the platform
        let neon = dm.add_part_styled(
            "NeonBeacon",
            Vec3::new(14.0, 7.5, -22.0),
            Vec3::new(2.5, 2.5, 2.5),
            ident,
            [0.15, 0.92, 1.0],
            0.0,
            true,
            PartShape::Block,
            PartMaterial::Neon,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        dm.attach_part_instance("NeonBeacon", ws, (neon - 1) as usize);

        // Classic rolling SmoothPlastic ball
        let ball_id = dm.add_part_styled(
            "ClassicBall",
            Vec3::new(-7.0, 4.0, -5.0),
            Vec3::new(3.6, 3.6, 3.6),
            ident,
            [0.95, 0.80, 0.18],
            0.0,
            false,
            PartShape::Ball,
            PartMaterial::SmoothPlastic,
            SurfaceType::Smooth,
            SurfaceType::Smooth,
            DecalKind::None,
        );
        dm.attach_part_instance("ClassicBall", ws, (ball_id - 1) as usize);
        dm.tools = vec![
            ToolItem {
                name: "LinkedSword".into(),
                icon: ToolIconKind::Sword,
                texture_id: None,
                mesh_id: None,
                grip_pos: Vec3::ZERO,
                handle_color: [0.76, 0.80, 0.85],
                handle_size: Vec3::new(0.28, 0.28, 3.6),
                handle_shape: PartShape::Block,
            },
            ToolItem {
                name: "Slingshot".into(),
                icon: ToolIconKind::Slingshot,
                texture_id: None,
                mesh_id: None,
                grip_pos: Vec3::ZERO,
                handle_color: [0.46, 0.28, 0.14],
                handle_size: Vec3::new(0.45, 1.3, 0.35),
                handle_shape: PartShape::Block,
            },
            ToolItem {
                name: "Trowel".into(),
                icon: ToolIconKind::Trowel,
                texture_id: None,
                mesh_id: None,
                grip_pos: Vec3::ZERO,
                handle_color: [0.68, 0.72, 0.76],
                handle_size: Vec3::new(0.5, 0.25, 1.8),
                handle_shape: PartShape::Block,
            },
        ];
        dm
    }

    fn attach_part_instance(&mut self, name: &str, parent_idx: usize, part_index: usize) -> usize {
        let idx = self.instances.len();
        self.instances.push(InstanceNode {
            name: name.to_string(),
            class_name: "Part".into(),
            parent: Some(parent_idx),
            children: Vec::new(),
            part_index: Some(part_index),
            script_index: None,
            disabled: false,
            value_num: 0.0,
            value_str: String::new(),
            value_bool: false,
        });
        self.instances[parent_idx].children.push(idx);
        idx
    }

    pub fn add_part(
        &mut self,
        name: impl Into<String>,
        position: Vec3,
        size: Vec3,
        color: [f32; 3],
        anchored: bool,
    ) -> u64 {
        self.add_part_full(
            name,
            position,
            size,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            color,
            0.0,
            anchored,
        )
    }

    pub fn add_part_full(
        &mut self,
        name: impl Into<String>,
        position: Vec3,
        size: Vec3,
        rotation: [[f32; 3]; 3],
        color: [f32; 3],
        transparency: f32,
        anchored: bool,
    ) -> u64 {
        self.add_part_shaped(
            name,
            position,
            size,
            rotation,
            color,
            transparency,
            anchored,
            PartShape::Block,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_part_shaped(
        &mut self,
        name: impl Into<String>,
        position: Vec3,
        size: Vec3,
        rotation: [[f32; 3]; 3],
        color: [f32; 3],
        transparency: f32,
        anchored: bool,
        shape: PartShape,
    ) -> u64 {
        let (top_surface, bottom_surface) = match shape {
            PartShape::Ball | PartShape::Cylinder | PartShape::Head | PartShape::BevelBlock => {
                (SurfaceType::Smooth, SurfaceType::Smooth)
            }
            _ => (SurfaceType::Studs, SurfaceType::Inlet),
        };
        self.add_part_styled(
            name,
            position,
            size,
            rotation,
            color,
            transparency,
            anchored,
            shape,
            PartMaterial::Plastic,
            top_surface,
            bottom_surface,
            DecalKind::None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_part_styled(
        &mut self,
        name: impl Into<String>,
        position: Vec3,
        size: Vec3,
        rotation: [[f32; 3]; 3],
        color: [f32; 3],
        transparency: f32,
        anchored: bool,
        shape: PartShape,
        material: PartMaterial,
        top_surface: SurfaceType,
        bottom_surface: SurfaceType,
        decal: DecalKind,
    ) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.parts.push(Part {
            id,
            name: name.into(),
            position,
            size,
            rotation,
            color,
            transparency,
            anchored,
            shape,
            material,
            top_surface,
            bottom_surface,
            decal,
        });
        id
    }

    /// Load a binary `.rbxl` or XML `.rbxlx` place file from bytes.
    pub fn from_place_bytes(bytes: &[u8]) -> Result<Self> {
        let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
        let trimmed = bytes
            .iter()
            .position(|b| !b.is_ascii_whitespace())
            .map_or(bytes, |i| &bytes[i..]);

        let dom = if bytes.starts_with(b"<roblox!") {
            rbx_binary::from_reader(Cursor::new(bytes))
                .context("failed to parse binary .rbxl")?
        } else if trimmed.starts_with(b"<roblox!") {
            rbx_binary::from_reader(Cursor::new(trimmed))
                .context("failed to parse binary .rbxl")?
        } else {
            rbx_xml::from_reader_default(Cursor::new(trimmed))
                .or_else(|xml_err| {
                    rbx_binary::from_reader(Cursor::new(bytes)).map_err(|_| xml_err)
                })
                .context("failed to parse XML .rbxlx / binary .rbxl")?
        };

        let mut dm = Self::default();
        dm.collect_instances(&dom, dom.root_ref(), None, false);
        dm.ensure_service("Workspace");
        dm.ensure_service("Players");
        dm.ensure_service("Lighting");
        dm.ensure_service("Debris");
        dm.ensure_service("StarterGui");
        dm.ensure_service("StarterPack");
        dm.ensure_service("RunService");
        Ok(dm)
    }

    fn collect_instances(
        &mut self,
        dom: &WeakDom,
        referent: rbx_dom_weak::types::Ref,
        parent_idx: Option<usize>,
        in_workspace: bool,
    ) {
        let Some(inst) = dom.get_by_ref(referent) else {
            return;
        };

        let child_in_workspace = in_workspace || inst.class.as_str() == "Workspace";

        let cur_idx = if parent_idx.is_none() {
            // Root WeakDom node maps to DataModel (index 0)
            0
        } else {
            let mut part_index = None;
            let mut script_index = None;
            let mut disabled = false;

            match inst.class.as_str() {
                "Part" | "WedgePart" | "CornerWedgePart" | "TrussPart" | "SpawnLocation"
                | "Seat" | "VehicleSeat" | "MeshPart" if in_workspace => {
                    let (pos, rot, size, color, transparency, mut anchored) = extract_part_props(inst);
                    if inst.class.as_str() == "SpawnLocation" {
                        self.spawn_points.push(pos + Vec3::new(0.0, size.y * 0.5 + 3.0, 0.0));
                    }
                    if !anchored {
                        let has_weld = inst.children().iter().any(|&c| {
                            dom.get_by_ref(c).is_some_and(|ch| {
                                matches!(
                                    ch.class.as_str(),
                                    "Weld" | "Snap" | "ManualWeld" | "Glue" | "Motor" | "Motor6D"
                                )
                            })
                        });
                        if has_weld {
                            anchored = true;
                        }
                    }
                    let shape = match inst.class.as_str() {
                        "WedgePart" | "CornerWedgePart" => PartShape::Wedge,
                        _ => match inst
                            .properties
                            .get(&ustr("shape"))
                            .or_else(|| inst.properties.get(&ustr("Shape")))
                        {
                            Some(Variant::Enum(e)) => match e.to_u32() {
                                0 => PartShape::Ball,
                                2 => PartShape::Cylinder,
                                _ => PartShape::Block,
                            },
                            Some(Variant::Int32(n)) => match *n {
                                0 => PartShape::Ball,
                                2 => PartShape::Cylinder,
                                _ => PartShape::Block,
                            },
                            _ => PartShape::Block,
                        },
                    };
                    let material = match inst.properties.get(&ustr("Material")) {
                        Some(Variant::Enum(e)) => PartMaterial::from_roblox_enum(e.to_u32()),
                        Some(Variant::Int32(n)) => PartMaterial::from_roblox_enum(*n as u32),
                        _ => PartMaterial::Plastic,
                    };
                    let default_top = if material == PartMaterial::Plastic
                        && matches!(shape, PartShape::Block | PartShape::Wedge)
                    {
                        SurfaceType::Studs
                    } else {
                        SurfaceType::Smooth
                    };
                    let default_bot = if material == PartMaterial::Plastic
                        && matches!(shape, PartShape::Block | PartShape::Wedge)
                    {
                        SurfaceType::Inlet
                    } else {
                        SurfaceType::Smooth
                    };
                    let top_surface = match inst.properties.get(&ustr("TopSurface")) {
                        Some(Variant::Enum(e)) => SurfaceType::from_roblox_enum(e.to_u32()),
                        Some(Variant::Int32(n)) => SurfaceType::from_roblox_enum(*n as u32),
                        _ => default_top,
                    };
                    let bottom_surface = match inst.properties.get(&ustr("BottomSurface")) {
                        Some(Variant::Enum(e)) => SurfaceType::from_roblox_enum(e.to_u32()),
                        Some(Variant::Int32(n)) => SurfaceType::from_roblox_enum(*n as u32),
                        _ => default_bot,
                    };
                    let mut final_shape = shape;
                    let mut decal = if inst.class.as_str() == "SpawnLocation" {
                        DecalKind::Spawn
                    } else {
                        DecalKind::None
                    };
                    // Detect SpecialMesh Head (MeshType == 0) or Face decal on NPCs in .rbxl places
                    for &c in inst.children() {
                        if let Some(ch) = dom.get_by_ref(c) {
                            if ch.class.as_str() == "SpecialMesh" {
                                let mesh_type = match ch.properties.get(&ustr("MeshType")) {
                                    Some(Variant::Enum(e)) => Some(e.to_u32()),
                                    Some(Variant::Int32(n)) => Some(*n as u32),
                                    _ => None,
                                };
                                match mesh_type {
                                    Some(0) => final_shape = PartShape::Head,
                                    Some(2) => final_shape = PartShape::Wedge,
                                    Some(3) => final_shape = PartShape::Ball,
                                    Some(4) => final_shape = PartShape::Cylinder,
                                    _ => {}
                                }
                            } else if ch.class.as_str() == "Decal" && ch.name.eq_ignore_ascii_case("face") {
                                decal = DecalKind::ClassicFace;
                            }
                        }
                    }
                    self.add_part_styled(
                        inst.name.clone(),
                        pos,
                        size,
                        rot,
                        color,
                        transparency,
                        anchored,
                        final_shape,
                        material,
                        top_surface,
                        bottom_surface,
                        decal,
                    );
                    part_index = Some(self.parts.len() - 1);
                }
                "Sky" => {
                    self.sky.skybox_bk = extract_content_prop(inst, "SkyboxBk");
                    self.sky.skybox_dn = extract_content_prop(inst, "SkyboxDn");
                    self.sky.skybox_ft = extract_content_prop(inst, "SkyboxFt");
                    self.sky.skybox_lf = extract_content_prop(inst, "SkyboxLf");
                    self.sky.skybox_rt = extract_content_prop(inst, "SkyboxRt");
                    self.sky.skybox_up = extract_content_prop(inst, "SkyboxUp");
                    if let Some(Variant::Bool(show)) =
                        inst.properties.get(&ustr("CelestialBodiesShown"))
                    {
                        self.sky.celestial_bodies_shown = *show;
                    }
                }
                "Lighting" => {
                    if let Some(Variant::Color3(c)) = inst
                        .properties
                        .get(&ustr("OutdoorAmbient"))
                        .or_else(|| inst.properties.get(&ustr("Ambient")))
                    {
                        if c.r + c.g + c.b > 0.05 {
                            self.sky.ambient = [c.r.clamp(0.15, 0.8), c.g.clamp(0.15, 0.8), c.b.clamp(0.15, 0.8)];
                        }
                    }
                    if let Some(Variant::Float32(b)) = inst.properties.get(&ustr("Brightness")) {
                        self.sky.brightness = b.clamp(0.4, 2.5);
                    }
                    if let Some(Variant::Color3(c)) = inst.properties.get(&ustr("ColorShift_Top")) {
                        if c.r + c.g + c.b > 0.05 {
                            self.sky.sky_top = [c.r, c.g, c.b];
                        }
                    }
                    if let Some(Variant::Color3(c)) = inst.properties.get(&ustr("FogColor")) {
                        if c.r + c.g + c.b > 0.05 {
                            self.sky.sky_bottom = [c.r, c.g, c.b];
                        }
                    }
                    if let Some(Variant::Float32(fs)) = inst.properties.get(&ustr("FogStart")) {
                        self.sky.fog_start = fs.max(40.0);
                    }
                    if let Some(Variant::Float32(fe)) = inst.properties.get(&ustr("FogEnd")) {
                        self.sky.fog_end = fe.max(self.sky.fog_start + 80.0);
                    }
                    if let Some(Variant::Float32(ct)) = inst.properties.get(&ustr("ClockTime")) {
                        let angle = ((*ct - 6.0) / 12.0) * std::f32::consts::PI;
                        self.sky.sun_direction =
                            Vec3::new(angle.cos(), angle.sin().max(0.15), 0.35).normalize();
                        if *ct < 6.0 || *ct > 19.0 {
                            self.sky.sky_top = [0.04, 0.06, 0.14];
                            self.sky.sky_bottom = [0.10, 0.13, 0.24];
                        }
                    }
                }
                "Tool" | "HopperBin" => {
                    let in_starter_pack = parent_idx
                        .and_then(|p| self.instances.get(p))
                        .is_some_and(|p| p.class_name == "StarterPack");
                    if in_starter_pack
                        && self.tools.len() < 6
                        && !self.tools.iter().any(|t| t.name == inst.name)
                    {
                        let texture_id = extract_content_prop(inst, "TextureId");
                        let grip_pos = match inst.properties.get(&ustr("GripPos")) {
                            Some(Variant::Vector3(v)) => Vec3::new(v.x, v.y, v.z),
                            _ => Vec3::ZERO,
                        };
                        let mut mesh_id: Option<String> = None;
                        let mut handle_part_props = None;
                        for &c in inst.children() {
                            if let Some(ch) = dom.get_by_ref(c) {
                                if ch.name.eq_ignore_ascii_case("Handle") {
                                    let (_, _, sz, col, _, _) = extract_part_props(ch);
                                    for &gc in ch.children() {
                                        if let Some(g) = dom.get_by_ref(gc) {
                                            if g.class.as_str() == "SpecialMesh" {
                                                mesh_id = extract_content_prop(g, "MeshId");
                                                break;
                                            }
                                        }
                                    }
                                    handle_part_props = Some((sz, col, mesh_id.is_some()));
                                    break;
                                }
                            }
                        }
                        let icon = ToolIconKind::from_name_and_texture(
                            &inst.name,
                            texture_id.as_deref(),
                            mesh_id.as_deref(),
                        );
                        let mut handle_color = match icon {
                            ToolIconKind::Sword => [0.76, 0.80, 0.86],
                            ToolIconKind::Slingshot => [0.46, 0.28, 0.14],
                            ToolIconKind::RocketLauncher => [0.28, 0.38, 0.22],
                            ToolIconKind::Trowel => [0.68, 0.72, 0.78],
                            ToolIconKind::Bomb => [0.12, 0.12, 0.14],
                            ToolIconKind::Superball => [0.85, 0.12, 0.10],
                            ToolIconKind::Generic => [0.64, 0.64, 0.68],
                        };
                        let mut handle_size = match icon {
                            ToolIconKind::Sword => Vec3::new(0.24, 0.28, 3.6),
                            ToolIconKind::Slingshot => Vec3::new(0.45, 1.3, 0.35),
                            ToolIconKind::RocketLauncher => Vec3::new(0.65, 0.65, 3.2),
                            ToolIconKind::Trowel => Vec3::new(0.55, 0.25, 1.8),
                            ToolIconKind::Bomb => Vec3::new(1.2, 1.2, 1.2),
                            ToolIconKind::Superball => Vec3::new(1.3, 1.3, 1.3),
                            ToolIconKind::Generic => Vec3::new(0.5, 0.5, 1.6),
                        };
                        let handle_shape = match icon {
                            ToolIconKind::Bomb | ToolIconKind::Superball => PartShape::Ball,
                            ToolIconKind::RocketLauncher => PartShape::Cylinder,
                            _ => PartShape::Block,
                        };
                        if let Some((sz, col, has_mesh)) = handle_part_props {
                            handle_color = col;
                            if !has_mesh && sz.length_squared() > 0.05 && sz.length() < 8.0 {
                                handle_size = sz;
                            }
                        }
                        self.tools.push(ToolItem {
                            name: inst.name.clone(),
                            icon,
                            texture_id,
                            mesh_id,
                            grip_pos,
                            handle_color,
                            handle_size,
                            handle_shape,
                        });
                    }
                }
                "Script" | "LocalScript" => {
                    if let Some(Variant::Bool(d)) = inst.properties.get(&ustr("Disabled")) {
                        disabled = *d;
                    }
                    if let Some(Variant::String(src)) = inst.properties.get(&ustr("Source")) {
                        script_index = Some(self.scripts.len());
                        self.scripts.push((inst.name.clone(), src.clone()));
                    }
                }
                _ => {}
            }

            let value_num = match inst.properties.get(&ustr("Value")) {
                Some(Variant::Int32(n)) => *n as f64,
                Some(Variant::Int64(n)) => *n as f64,
                Some(Variant::Float32(n)) => *n as f64,
                Some(Variant::Float64(n)) => *n,
                _ => 0.0,
            };
            let value_str = match inst.properties.get(&ustr("Value")) {
                Some(Variant::String(s)) => s.clone(),
                _ => String::new(),
            };
            let value_bool = matches!(inst.properties.get(&ustr("Value")), Some(Variant::Bool(true)));

            let idx = self.instances.len();
            self.instances.push(InstanceNode {
                name: inst.name.clone(),
                class_name: inst.class.to_string(),
                parent: parent_idx,
                children: Vec::new(),
                part_index,
                script_index,
                disabled,
                value_num,
                value_str,
                value_bool,
            });
            if let Some(p) = parent_idx {
                self.instances[p].children.push(idx);
            }
            if script_index.is_some() {
                self.script_instances.push(idx);
            }
            idx
        };

        for &child in inst.children() {
            self.collect_instances(dom, child, Some(cur_idx), child_in_workspace);
        }
    }

    /// Execute loaded Luau scripts in a sandboxed VM with Roblox API bindings.
    #[cfg(feature = "luau")]
    pub fn run_luau_scripts_mut(&mut self) -> Result<Vec<String>> {
        luau_api::execute_scripts(self)
    }

    #[cfg(not(feature = "luau"))]
    pub fn run_luau_scripts_mut(&mut self) -> Result<Vec<String>> {
        Ok(vec![format!(
            "[Luau] skipped {} scripts (built without `luau` feature)",
            self.scripts.len()
        )])
    }

    pub fn run_luau_scripts(&self) -> Result<Vec<String>> {
        let mut copy = self.clone();
        copy.run_luau_scripts_mut()
    }
}

fn extract_part_props(inst: &Instance) -> (Vec3, [[f32; 3]; 3], Vec3, [f32; 3], f32, bool) {
    let (pos, rot) = match inst.properties.get(&ustr("CFrame")) {
        Some(Variant::CFrame(cf)) => (
            Vec3::new(cf.position.x, cf.position.y, cf.position.z),
            [
                [cf.orientation.x.x, cf.orientation.x.y, cf.orientation.x.z],
                [cf.orientation.y.x, cf.orientation.y.y, cf.orientation.y.z],
                [cf.orientation.z.x, cf.orientation.z.y, cf.orientation.z.z],
            ],
        ),
        _ => {
            let p = match inst.properties.get(&ustr("Position")) {
                Some(Variant::Vector3(v)) => Vec3::new(v.x, v.y, v.z),
                _ => Vec3::ZERO,
            };
            (p, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
        }
    };

    let size = match inst.properties.get(&ustr("Size")) {
        Some(Variant::Vector3(v)) => Vec3::new(v.x, v.y, v.z),
        _ => match inst.properties.get(&ustr("size")) {
            Some(Variant::Vector3(v)) => Vec3::new(v.x, v.y, v.z),
            _ => Vec3::new(4.0, 1.2, 2.0),
        },
    };

    let color = match inst.properties.get(&ustr("Color3uint8")) {
        Some(Variant::Color3uint8(c)) => [
            c.r as f32 / 255.0,
            c.g as f32 / 255.0,
            c.b as f32 / 255.0,
        ],
        _ => match inst.properties.get(&ustr("Color")) {
            Some(Variant::Color3(c)) => [c.r, c.g, c.b],
            Some(Variant::Color3uint8(c)) => [
                c.r as f32 / 255.0,
                c.g as f32 / 255.0,
                c.b as f32 / 255.0,
            ],
            _ => match inst.properties.get(&ustr("BrickColor")) {
                Some(Variant::BrickColor(bc)) => {
                    let c = bc.to_color3uint8();
                    [
                        c.r as f32 / 255.0,
                        c.g as f32 / 255.0,
                        c.b as f32 / 255.0,
                    ]
                }
                Some(Variant::Int32(n)) => {
                    if let Some(bc) = rbx_types::BrickColor::from_number(*n as u16) {
                        let c = bc.to_color3uint8();
                        [
                            c.r as f32 / 255.0,
                            c.g as f32 / 255.0,
                            c.b as f32 / 255.0,
                        ]
                    } else {
                        [0.64, 0.64, 0.64]
                    }
                }
                _ => [0.64, 0.64, 0.64],
            },
        },
    };

    let transparency = match inst.properties.get(&ustr("Transparency")) {
        Some(Variant::Float32(t)) => *t,
        _ => 0.0,
    };

    let anchored = matches!(
        inst.properties
            .get(&ustr("Anchored"))
            .or_else(|| inst.properties.get(&ustr("anchored"))),
        Some(Variant::Bool(true))
    );

    let _ = Mat4::IDENTITY;
    (pos, rot, size, color, transparency, anchored)
}

fn extract_content_prop(inst: &Instance, name: &str) -> Option<String> {
    match inst.properties.get(&ustr(name)) {
        Some(Variant::ContentId(c)) if !c.as_str().is_empty() => Some(c.as_str().to_string()),
        Some(Variant::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_xml_place_and_runs_luau() {
        let xml = br#"<roblox version="4">
            <Item class="Workspace">
                <Item class="Part">
                    <Properties>
                        <string name="Name">TestBlock</string>
                        <Vector3 name="size"><X>6</X><Y>2</Y><Z>6</Z></Vector3>
                        <bool name="Anchored">true</bool>
                    </Properties>
                </Item>
                <Item class="Script">
                    <Properties>
                        <string name="Name">Hello</string>
                        <ProtectedString name="Source">local x: number = 2 + 2</ProtectedString>
                    </Properties>
                </Item>
            </Item>
        </roblox>"#;

        let dm = DataModel::from_place_bytes(xml).expect("parse rbxlx");
        assert_eq!(dm.parts.len(), 1);
        assert_eq!(dm.parts[0].name, "TestBlock");
        assert_eq!(dm.scripts.len(), 1);

        let logs = dm.run_luau_scripts().expect("run luau");
        assert_eq!(logs.len(), 1);
        assert!(logs[0].contains("executed Hello"));
    }

    #[cfg(feature = "luau")]
    #[test]
    fn luau_instance_new_spawns_part_in_workspace() {
        let mut dm = DataModel::new_baseplate();
        dm.scripts.push((
            "Spawner".into(),
            r#"
            local p = Instance.new("Part")
            p.Name = "ScriptedBrick"
            p.Position = Vector3.new(10, 5, 0)
            p.Size = Vector3.new(4, 4, 4)
            p.Color = Color3.fromRGB(0, 255, 128)
            p.Anchored = true
            p.Parent = game:GetService("Workspace")
            "#
            .into(),
        ));
        let logs = dm.run_luau_scripts_mut().expect("run luau");
        assert!(logs[0].contains("executed Spawner"));
        let spawned = dm.parts.iter().find(|p| p.name == "ScriptedBrick").unwrap();
        assert_eq!(spawned.position, Vec3::new(10.0, 5.0, 0.0));
        assert!(spawned.anchored);
    }

    #[test]
    fn parses_roblox_materials_surfaces_and_head_decal() {
        let xml = br#"<roblox version="4">
            <Item class="Workspace">
                <Item class="Part">
                    <Properties>
                        <string name="Name">BrickWall</string>
                        <token name="Material">848</token>
                        <token name="TopSurface">0</token>
                        <bool name="Anchored">true</bool>
                    </Properties>
                </Item>
                <Item class="Part">
                    <Properties>
                        <string name="Name">Head</string>
                        <bool name="Anchored">true</bool>
                    </Properties>
                    <Item class="SpecialMesh">
                        <Properties>
                            <token name="MeshType">0</token>
                        </Properties>
                    </Item>
                    <Item class="Decal">
                        <Properties>
                            <string name="Name">face</string>
                        </Properties>
                    </Item>
                </Item>
                <Item class="SpawnLocation">
                    <Properties>
                        <string name="Name">Spawn</string>
                        <bool name="Anchored">true</bool>
                    </Properties>
                </Item>
            </Item>
        </roblox>"#;

        let dm = DataModel::from_place_bytes(xml).expect("parse rbxlx");
        let wall = dm.parts.iter().find(|p| p.name == "BrickWall").unwrap();
        assert_eq!(wall.material, PartMaterial::Brick);
        assert_eq!(wall.top_surface, SurfaceType::Smooth);

        let head = dm.parts.iter().find(|p| p.name == "Head").unwrap();
        assert_eq!(head.shape, PartShape::Head);
        assert_eq!(head.decal, DecalKind::ClassicFace);

        let spawn = dm.parts.iter().find(|p| p.name == "Spawn").unwrap();
        assert_eq!(spawn.decal, DecalKind::Spawn);
    }

    #[test]
    fn parses_starter_pack_tools_in_order_and_ignores_workspace_tools() {
        let xml = br#"<roblox version="4">
            <Item class="Workspace">
                <Item class="Tool">
                    <Properties>
                        <string name="Name">WorldPickupSword</string>
                    </Properties>
                </Item>
            </Item>
            <Item class="StarterPack">
                <Item class="Tool">
                    <Properties>
                        <string name="Name">LinkedTimebomb</string>
                        <Content name="TextureId"><url>rbxasset://Textures/Bomb.png</url></Content>
                    </Properties>
                </Item>
                <Item class="Tool">
                    <Properties>
                        <string name="Name">CustomGadget</string>
                    </Properties>
                </Item>
                <Item class="Tool">
                    <Properties>
                        <string name="Name">LinkedSword</string>
                    </Properties>
                </Item>
            </Item>
        </roblox>"#;

        let dm = DataModel::from_place_bytes(xml).expect("parse rbxlx");
        assert_eq!(dm.tools.len(), 3);
        assert_eq!(dm.tools[0].name, "LinkedTimebomb");
        assert_eq!(dm.tools[0].icon, ToolIconKind::Bomb);
        assert_eq!(dm.tools[1].name, "CustomGadget");
        assert_eq!(dm.tools[1].icon, ToolIconKind::Generic);
        assert_eq!(dm.tools[2].name, "LinkedSword");
        assert_eq!(dm.tools[2].icon, ToolIconKind::Sword);
    }
}
