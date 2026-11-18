#[cfg(feature = "luau")]
pub mod luau_api;
pub mod physics;

use std::io::Cursor;

use anyhow::{Context, Result};
use glam::{Mat4, Vec3};
use rbx_dom_weak::{types::Variant, ustr, Instance, WeakDom};
use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkyConfig {
    pub sky_top: [f32; 3],
    pub sky_bottom: [f32; 3],
    pub sun_direction: Vec3,
    pub skybox_bk: Option<String>,
    pub skybox_dn: Option<String>,
    pub skybox_ft: Option<String>,
    pub skybox_lf: Option<String>,
    pub skybox_rt: Option<String>,
    pub skybox_up: Option<String>,
    pub celestial_bodies_shown: bool,
}

impl Default for SkyConfig {
    fn default() -> Self {
        Self {
            sky_top: [0.47, 0.71, 0.95],
            sky_bottom: [0.69, 0.89, 0.99],
            sun_direction: Vec3::new(0.45, 1.0, 0.35).normalize(),
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

#[derive(Debug, Clone)]
pub struct DataModel {
    pub parts: Vec<Part>,
    pub spawn_points: Vec<Vec3>,
    pub scripts: Vec<(String, String)>,
    pub script_instances: Vec<usize>,
    pub instances: Vec<InstanceNode>,
    pub sky: SkyConfig,
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
        let base_id = dm.add_part(
            "Baseplate",
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(128.0, 2.0, 128.0),
            [0.39, 0.42, 0.45],
            true,
        );
        dm.attach_part_instance("Baseplate", ws, (base_id - 1) as usize);
        let spawn_id = dm.add_part(
            "SpawnBlock",
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(4.0, 4.0, 4.0),
            [0.85, 0.25, 0.25],
            true,
        );
        dm.attach_part_instance("SpawnBlock", ws, (spawn_id - 1) as usize);
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
        });
        id
    }

    /// Load a binary `.rbxl` or XML `.rbxlx` place file from bytes.
    pub fn from_place_bytes(bytes: &[u8]) -> Result<Self> {
        let dom = if bytes.starts_with(b"<roblox!") {
            rbx_binary::from_reader(Cursor::new(bytes))
                .context("failed to parse binary .rbxl")?
        } else {
            rbx_xml::from_reader_default(Cursor::new(bytes))
                .context("failed to parse XML .rbxlx")?
        };

        let mut dm = Self::default();
        dm.collect_instances(&dom, dom.root_ref(), None);
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
    ) {
        let Some(inst) = dom.get_by_ref(referent) else {
            return;
        };

        let cur_idx = if parent_idx.is_none() {
            // Root WeakDom node maps to DataModel (index 0)
            0
        } else {
            let mut part_index = None;
            let mut script_index = None;
            let mut disabled = false;

            match inst.class.as_str() {
                "Part" | "WedgePart" | "CornerWedgePart" | "TrussPart" | "SpawnLocation" => {
                    let (pos, rot, size, color, transparency, mut anchored) = extract_part_props(inst);
                    if inst.class.as_str() == "SpawnLocation" {
                        self.spawn_points.push(pos + Vec3::new(0.0, size.y * 0.5 + 3.5, 0.0));
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
                    self.add_part_full(
                        inst.name.clone(),
                        pos,
                        size,
                        rot,
                        color,
                        transparency,
                        anchored,
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
            self.collect_instances(dom, child, Some(cur_idx));
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
}
