use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use anyhow::Result;
use glam::Vec3;
use mlua::{Lua, MetaMethod, UserData, UserDataFields, UserDataMethods, Value};

use crate::{DataModel, InstanceNode};

const YIELD_SENTINEL: &str = "__revival_yield__";

#[derive(Clone, Copy)]
struct RbxVec3(Vec3);

impl UserData for RbxVec3 {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("X", |_, this| Ok(this.0.x));
        fields.add_field_method_get("Y", |_, this| Ok(this.0.y));
        fields.add_field_method_get("Z", |_, this| Ok(this.0.z));
        fields.add_field_method_get("x", |_, this| Ok(this.0.x));
        fields.add_field_method_get("y", |_, this| Ok(this.0.y));
        fields.add_field_method_get("z", |_, this| Ok(this.0.z));
        fields.add_field_method_get("Magnitude", |_, this| Ok(this.0.length()));
        fields.add_field_method_get("magnitude", |_, this| Ok(this.0.length()));
        fields.add_field_method_get("Unit", |_, this| Ok(RbxVec3(this.0.normalize_or_zero())));
        fields.add_field_method_get("unit", |_, this| Ok(RbxVec3(this.0.normalize_or_zero())));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Add, |_, this, other: mlua::UserDataRef<RbxVec3>| {
            Ok(RbxVec3(this.0 + other.0))
        });
        methods.add_meta_method(MetaMethod::Sub, |_, this, other: mlua::UserDataRef<RbxVec3>| {
            Ok(RbxVec3(this.0 - other.0))
        });
        methods.add_meta_method(MetaMethod::Mul, |_, this, rhs: Value| match rhs {
            Value::Number(n) => Ok(RbxVec3(this.0 * n as f32)),
            Value::Integer(n) => Ok(RbxVec3(this.0 * n as f32)),
            Value::UserData(ud) => {
                let other = ud.borrow::<RbxVec3>()?;
                Ok(RbxVec3(this.0 * other.0))
            }
            _ => Err(mlua::Error::RuntimeError("invalid Vector3 multiplication".into())),
        });
        methods.add_meta_method(MetaMethod::Div, |_, this, s: f32| Ok(RbxVec3(this.0 / s)));
        methods.add_meta_method(MetaMethod::Unm, |_, this, ()| Ok(RbxVec3(-this.0)));
    }
}

#[derive(Clone, Copy)]
struct RbxColor3([f32; 3]);

impl UserData for RbxColor3 {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("R", |_, this| Ok(this.0[0]));
        fields.add_field_method_get("G", |_, this| Ok(this.0[1]));
        fields.add_field_method_get("B", |_, this| Ok(this.0[2]));
        fields.add_field_method_get("r", |_, this| Ok(this.0[0]));
        fields.add_field_method_get("g", |_, this| Ok(this.0[1]));
        fields.add_field_method_get("b", |_, this| Ok(this.0[2]));
    }
}

#[derive(Clone, Copy)]
struct RbxCFrame(Vec3);

impl UserData for RbxCFrame {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Position", |_, this| Ok(RbxVec3(this.0)));
        fields.add_field_method_get("p", |_, this| Ok(RbxVec3(this.0)));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Mul, |lua, this, rhs: Value| match rhs {
            Value::UserData(ud) => {
                if let Ok(cf) = ud.borrow::<RbxCFrame>() {
                    Ok(Value::UserData(lua.create_userdata(RbxCFrame(this.0 + cf.0))?))
                } else if let Ok(v) = ud.borrow::<RbxVec3>() {
                    Ok(Value::UserData(lua.create_userdata(RbxVec3(this.0 + v.0))?))
                } else {
                    Ok(Value::UserData(lua.create_userdata(*this)?))
                }
            }
            _ => Ok(Value::UserData(lua.create_userdata(*this)?)),
        });
    }
}

#[derive(Clone, Copy)]
struct RbxConnection;

impl UserData for RbxConnection {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Disconnect", |_, _, ()| Ok(()));
        methods.add_method("disconnect", |_, _, ()| Ok(()));
    }
}

#[derive(Clone, Copy)]
struct RbxSignal;

impl UserData for RbxSignal {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Connect", |_, _, _cb: Value| Ok(RbxConnection));
        methods.add_method("connect", |_, _, _cb: Value| Ok(RbxConnection));
        methods.add_method("Wait", |_, _, ()| -> mlua::Result<()> {
            Err(mlua::Error::RuntimeError(YIELD_SENTINEL.into()))
        });
        methods.add_method("wait", |_, _, ()| -> mlua::Result<()> {
            Err(mlua::Error::RuntimeError(YIELD_SENTINEL.into()))
        });
    }
}

#[derive(Clone)]
struct RbxInstance {
    dm: Rc<RefCell<DataModel>>,
    idx: usize,
}

impl RbxInstance {
    fn find_child_rec(dm: &DataModel, parent_idx: usize, name: &str, recursive: bool) -> Option<usize> {
        let node = dm.instances.get(parent_idx)?;
        for &c in &node.children {
            if let Some(child) = dm.instances.get(c) {
                if child.name == name {
                    return Some(c);
                }
            }
        }
        if recursive {
            for &c in &node.children {
                if let Some(found) = Self::find_child_rec(dm, c, name, true) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn clone_subtree(dm: &mut DataModel, src_idx: usize, new_parent: Option<usize>) -> usize {
        let src = dm.instances[src_idx].clone();
        let part_index = src.part_index.map(|pi| {
            let p = dm.parts[pi].clone();
            dm.add_part_full(
                p.name,
                p.position,
                p.size,
                p.rotation,
                p.color,
                p.transparency,
                p.anchored,
            );
            dm.parts.len() - 1
        });
        let new_idx = dm.instances.len();
        dm.instances.push(InstanceNode {
            name: src.name,
            class_name: src.class_name,
            parent: new_parent,
            children: Vec::new(),
            part_index,
            script_index: src.script_index,
            disabled: src.disabled,
            value_num: src.value_num,
            value_str: src.value_str,
            value_bool: src.value_bool,
        });
        if let Some(p) = new_parent {
            dm.instances[p].children.push(new_idx);
        }
        for c in src.children {
            Self::clone_subtree(dm, c, Some(new_idx));
        }
        new_idx
    }

    fn reparent(dm: &mut DataModel, idx: usize, new_parent: Option<usize>) {
        if let Some(old_p) = dm.instances[idx].parent {
            dm.instances[old_p].children.retain(|&c| c != idx);
        }
        dm.instances[idx].parent = new_parent;
        if let Some(np) = new_parent {
            if !dm.instances[np].children.contains(&idx) {
                dm.instances[np].children.push(idx);
            }
        }
    }
}

impl UserData for RbxInstance {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Name", |_, this| {
            Ok(this.dm.borrow().instances[this.idx].name.clone())
        });
        fields.add_field_method_set("Name", |_, this, val: String| {
            let mut dm = this.dm.borrow_mut();
            let part_idx = dm.instances[this.idx].part_index;
            dm.instances[this.idx].name = val.clone();
            if let Some(pi) = part_idx {
                dm.parts[pi].name = val;
            }
            Ok(())
        });

        fields.add_field_method_get("ClassName", |_, this| {
            Ok(this.dm.borrow().instances[this.idx].class_name.clone())
        });
        fields.add_field_method_get("className", |_, this| {
            Ok(this.dm.borrow().instances[this.idx].class_name.clone())
        });

        fields.add_field_method_get("Parent", |_, this| {
            let parent = this.dm.borrow().instances[this.idx].parent;
            Ok(parent.map(|p| RbxInstance {
                dm: this.dm.clone(),
                idx: p,
            }))
        });
        fields.add_field_method_set("Parent", |_, this, val: Value| {
            let new_parent = match val {
                Value::Nil => None,
                Value::UserData(ud) => ud.borrow::<RbxInstance>().ok().map(|p| p.idx),
                _ => None,
            };
            RbxInstance::reparent(&mut this.dm.borrow_mut(), this.idx, new_parent);
            Ok(())
        });

        fields.add_field_method_get("Disabled", |_, this| {
            Ok(this.dm.borrow().instances[this.idx].disabled)
        });
        fields.add_field_method_set("Disabled", |_, this, val: bool| {
            this.dm.borrow_mut().instances[this.idx].disabled = val;
            Ok(())
        });

        fields.add_field_method_get("Position", |_, this| {
            let dm = this.dm.borrow();
            let pos = dm.instances[this.idx]
                .part_index
                .map(|pi| dm.parts[pi].position)
                .unwrap_or(Vec3::ZERO);
            Ok(RbxVec3(pos))
        });
        fields.add_field_method_set("Position", |_, this, val: mlua::UserDataRef<RbxVec3>| {
            let mut dm = this.dm.borrow_mut();
            if let Some(pi) = dm.instances[this.idx].part_index {
                dm.parts[pi].position = val.0;
            }
            Ok(())
        });
        fields.add_field_method_get("position", |_, this| {
            let dm = this.dm.borrow();
            let pos = dm.instances[this.idx]
                .part_index
                .map(|pi| dm.parts[pi].position)
                .unwrap_or(Vec3::ZERO);
            Ok(RbxVec3(pos))
        });
        fields.add_field_method_set("position", |_, this, val: mlua::UserDataRef<RbxVec3>| {
            let mut dm = this.dm.borrow_mut();
            if let Some(pi) = dm.instances[this.idx].part_index {
                dm.parts[pi].position = val.0;
            }
            Ok(())
        });

        fields.add_field_method_get("Texture", |_, this| {
            Ok(this.dm.borrow().instances[this.idx].value_str.clone())
        });
        fields.add_field_method_set("Texture", |_, this, val: String| {
            this.dm.borrow_mut().instances[this.idx].value_str = val;
            Ok(())
        });
        fields.add_field_method_get("Text", |_, this| {
            Ok(this.dm.borrow().instances[this.idx].value_str.clone())
        });
        fields.add_field_method_set("Text", |_, this, val: String| {
            this.dm.borrow_mut().instances[this.idx].value_str = val;
            Ok(())
        });
        fields.add_field_method_get("Enabled", |_, this| {
            Ok(this.dm.borrow().instances[this.idx].value_bool)
        });
        fields.add_field_method_set("Enabled", |_, this, val: bool| {
            this.dm.borrow_mut().instances[this.idx].value_bool = val;
            Ok(())
        });

        fields.add_field_method_get("CFrame", |_, this| {
            let dm = this.dm.borrow();
            let pos = dm.instances[this.idx]
                .part_index
                .map(|pi| dm.parts[pi].position)
                .unwrap_or(Vec3::ZERO);
            Ok(RbxCFrame(pos))
        });
        fields.add_field_method_set("CFrame", |_, this, val: mlua::UserDataRef<RbxCFrame>| {
            let mut dm = this.dm.borrow_mut();
            if let Some(pi) = dm.instances[this.idx].part_index {
                dm.parts[pi].position = val.0;
            }
            Ok(())
        });

        fields.add_field_method_get("Size", |_, this| {
            let dm = this.dm.borrow();
            let size = dm.instances[this.idx]
                .part_index
                .map(|pi| dm.parts[pi].size)
                .unwrap_or(Vec3::new(4.0, 1.2, 2.0));
            Ok(RbxVec3(size))
        });
        fields.add_field_method_set("Size", |_, this, val: mlua::UserDataRef<RbxVec3>| {
            let mut dm = this.dm.borrow_mut();
            if let Some(pi) = dm.instances[this.idx].part_index {
                dm.parts[pi].size = val.0;
            }
            Ok(())
        });

        fields.add_field_method_get("Color", |_, this| {
            let dm = this.dm.borrow();
            let color = dm.instances[this.idx]
                .part_index
                .map(|pi| dm.parts[pi].color)
                .unwrap_or([0.64, 0.64, 0.64]);
            Ok(RbxColor3(color))
        });
        fields.add_field_method_set("Color", |_, this, val: mlua::UserDataRef<RbxColor3>| {
            let mut dm = this.dm.borrow_mut();
            if let Some(pi) = dm.instances[this.idx].part_index {
                dm.parts[pi].color = val.0;
            }
            Ok(())
        });

        fields.add_field_method_get("Anchored", |_, this| {
            let dm = this.dm.borrow();
            Ok(dm.instances[this.idx]
                .part_index
                .map(|pi| dm.parts[pi].anchored)
                .unwrap_or(false))
        });
        fields.add_field_method_set("Anchored", |_, this, val: bool| {
            let mut dm = this.dm.borrow_mut();
            if let Some(pi) = dm.instances[this.idx].part_index {
                dm.parts[pi].anchored = val;
            }
            Ok(())
        });

        fields.add_field_method_get("Transparency", |_, this| {
            let dm = this.dm.borrow();
            Ok(dm.instances[this.idx]
                .part_index
                .map(|pi| dm.parts[pi].transparency)
                .unwrap_or(0.0))
        });
        fields.add_field_method_set("Transparency", |_, this, val: f32| {
            let mut dm = this.dm.borrow_mut();
            if let Some(pi) = dm.instances[this.idx].part_index {
                dm.parts[pi].transparency = val;
            }
            Ok(())
        });

        fields.add_field_method_get("CanCollide", |_, _| Ok(true));
        fields.add_field_method_set("CanCollide", |_, _, _: bool| Ok(()));

        fields.add_field_method_get("Value", |lua, this| {
            let dm = this.dm.borrow();
            let node = &dm.instances[this.idx];
            if node.class_name == "StringValue" {
                Ok(Value::String(lua.create_string(&node.value_str)?))
            } else if node.class_name == "BoolValue" {
                Ok(Value::Boolean(node.value_bool))
            } else {
                Ok(Value::Number(node.value_num))
            }
        });
        fields.add_field_method_set("Value", |_, this, val: Value| {
            let mut dm = this.dm.borrow_mut();
            let node = &mut dm.instances[this.idx];
            match val {
                Value::Number(n) => node.value_num = n,
                Value::Integer(n) => node.value_num = n as f64,
                Value::Boolean(b) => node.value_bool = b,
                Value::String(s) => node.value_str = s.to_str()?.to_string(),
                _ => {}
            }
            Ok(())
        });

        // Common Roblox signals
        for sig in [
            "Touched",
            "TouchEnded",
            "ChildAdded",
            "ChildRemoved",
            "Changed",
            "Equipped",
            "Unequipped",
            "Activated",
            "Clicked",
            "MouseClick",
            "PlayerAdded",
            "PlayerRemoving",
            "CharacterAdded",
            "Died",
            "Stepped",
            "Heartbeat",
            "RenderStepped",
        ] {
            fields.add_field_method_get(sig, |_, _| Ok(RbxSignal));
        }
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetService", |_, this, name: String| {
            let svc_idx = this.dm.borrow_mut().ensure_service(&name);
            Ok(RbxInstance {
                dm: this.dm.clone(),
                idx: svc_idx,
            })
        });
        methods.add_method("service", |_, this, name: String| {
            let svc_idx = this.dm.borrow_mut().ensure_service(&name);
            Ok(RbxInstance {
                dm: this.dm.clone(),
                idx: svc_idx,
            })
        });

        let find_first = |this: &RbxInstance, name: String, rec: Option<bool>| {
            let dm = this.dm.borrow();
            Ok(
                RbxInstance::find_child_rec(&dm, this.idx, &name, rec.unwrap_or(false)).map(
                    |c| RbxInstance {
                        dm: this.dm.clone(),
                        idx: c,
                    },
                ),
            )
        };
        methods.add_method("FindFirstChild", move |_, this, (n, r): (String, Option<bool>)| {
            find_first(this, n, r)
        });
        methods.add_method("findFirstChild", move |_, this, (n, r): (String, Option<bool>)| {
            find_first(this, n, r)
        });
        methods.add_method("WaitForChild", move |_, this, n: String| find_first(this, n, Some(false)));

        let get_children = |this: &RbxInstance| {
            let dm = this.dm.borrow();
            let list: Vec<RbxInstance> = dm.instances[this.idx]
                .children
                .iter()
                .map(|&c| RbxInstance {
                    dm: this.dm.clone(),
                    idx: c,
                })
                .collect();
            Ok(list)
        };
        methods.add_method("GetChildren", move |_, this, ()| get_children(this));
        methods.add_method("getChildren", move |_, this, ()| get_children(this));
        methods.add_method("children", move |_, this, ()| get_children(this));

        methods.add_method("IsA", |_, this, cls: String| {
            let dm = this.dm.borrow();
            let c = &dm.instances[this.idx].class_name;
            Ok(c == &cls || (cls == "BasePart" && matches!(c.as_str(), "Part" | "WedgePart" | "CornerWedgePart" | "TrussPart" | "SpawnLocation")))
        });
        methods.add_method("isA", |_, this, cls: String| {
            let dm = this.dm.borrow();
            let c = &dm.instances[this.idx].class_name;
            Ok(c == &cls || (cls == "BasePart" && matches!(c.as_str(), "Part" | "WedgePart" | "CornerWedgePart" | "TrussPart" | "SpawnLocation")))
        });

        methods.add_method("Clone", |_, this, ()| {
            let idx = RbxInstance::clone_subtree(&mut this.dm.borrow_mut(), this.idx, None);
            Ok(RbxInstance {
                dm: this.dm.clone(),
                idx,
            })
        });
        methods.add_method("clone", |_, this, ()| {
            let idx = RbxInstance::clone_subtree(&mut this.dm.borrow_mut(), this.idx, None);
            Ok(RbxInstance {
                dm: this.dm.clone(),
                idx,
            })
        });

        methods.add_method("Destroy", |_, this, ()| {
            RbxInstance::reparent(&mut this.dm.borrow_mut(), this.idx, None);
            Ok(())
        });
        methods.add_method("destroy", |_, this, ()| {
            RbxInstance::reparent(&mut this.dm.borrow_mut(), this.idx, None);
            Ok(())
        });
        methods.add_method("Remove", |_, this, ()| {
            RbxInstance::reparent(&mut this.dm.borrow_mut(), this.idx, None);
            Ok(())
        });
        methods.add_method("remove", |_, this, ()| {
            RbxInstance::reparent(&mut this.dm.borrow_mut(), this.idx, None);
            Ok(())
        });
        methods.add_method("ClearAllChildren", |_, this, ()| {
            let mut dm = this.dm.borrow_mut();
            let children = std::mem::take(&mut dm.instances[this.idx].children);
            for c in children {
                dm.instances[c].parent = None;
            }
            Ok(())
        });
        methods.add_method("AddItem", |_, _, (_inst, _ttl): (Value, Option<f64>)| Ok(()));
        methods.add_method("MakeJoints", |_, _, ()| Ok(()));
        methods.add_method("makeJoints", |_, _, ()| Ok(()));
        methods.add_method("GetMinutesAfterMidnight", |_, _, ()| Ok(720.0_f64));
        methods.add_method("SetMinutesAfterMidnight", |_, _, _m: f64| Ok(()));

        // Fallback child lookup: game.Workspace, workspace.Tower, script.Parent.Brick, etc.
        methods.add_meta_method(MetaMethod::Index, |lua, this, key: String| {
            let dm = this.dm.borrow();
            if let Some(c) = RbxInstance::find_child_rec(&dm, this.idx, &key, false) {
                return Ok(Value::UserData(lua.create_userdata(RbxInstance {
                    dm: this.dm.clone(),
                    idx: c,
                })?));
            }
            Ok(Value::Nil)
        });
    }
}

pub fn execute_scripts(dm: &mut DataModel) -> Result<Vec<String>> {
    let scripts = dm.scripts.clone();
    let script_instances = dm.script_instances.clone();
    let ws_idx = dm.ensure_service("Workspace");
    let shared_dm = Rc::new(RefCell::new(std::mem::take(dm)));

    let lua = Lua::new();
    lua.sandbox(true)
        .map_err(|e| anyhow::anyhow!("failed to enable Luau sandbox: {e}"))?;

    let wait_calls = Rc::new(Cell::new(0u32));
    let vm_steps = Rc::new(Cell::new(0u32));
    let vm_steps_cb = vm_steps.clone();
    lua.set_interrupt(move |_| {
        let s = vm_steps_cb.get().saturating_add(1);
        vm_steps_cb.set(s);
        if s > 20_000 {
            Err(mlua::Error::RuntimeError(YIELD_SENTINEL.into()))
        } else {
            Ok(mlua::VmState::Continue)
        }
    });

    let globals = lua.globals();

    // Vector3
    let vec3_tbl = lua.create_table().map_err(|e| anyhow::anyhow!("{e}"))?;
    vec3_tbl
        .set(
            "new",
            lua.create_function(|_, (x, y, z): (Option<f32>, Option<f32>, Option<f32>)| {
                Ok(RbxVec3(Vec3::new(
                    x.unwrap_or(0.0),
                    y.unwrap_or(0.0),
                    z.unwrap_or(0.0),
                )))
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    globals.set("Vector3", vec3_tbl).map_err(|e| anyhow::anyhow!("{e}"))?;

    // Color3
    let color3_tbl = lua.create_table().map_err(|e| anyhow::anyhow!("{e}"))?;
    color3_tbl
        .set(
            "new",
            lua.create_function(|_, (r, g, b): (Option<f32>, Option<f32>, Option<f32>)| {
                Ok(RbxColor3([
                    r.unwrap_or(0.0),
                    g.unwrap_or(0.0),
                    b.unwrap_or(0.0),
                ]))
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    color3_tbl
        .set(
            "fromRGB",
            lua.create_function(|_, (r, g, b): (f32, f32, f32)| {
                Ok(RbxColor3([r / 255.0, g / 255.0, b / 255.0]))
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    globals.set("Color3", color3_tbl).map_err(|e| anyhow::anyhow!("{e}"))?;

    // BrickColor
    let bc_tbl = lua.create_table().map_err(|e| anyhow::anyhow!("{e}"))?;
    bc_tbl
        .set(
            "new",
            lua.create_function(|_, _arg: Value| Ok(RbxColor3([0.64, 0.64, 0.64])))
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    bc_tbl
        .set(
            "Random",
            lua.create_function(|_, ()| Ok(RbxColor3([0.75, 0.35, 0.25])))
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    globals.set("BrickColor", bc_tbl).map_err(|e| anyhow::anyhow!("{e}"))?;

    // CFrame
    let cframe_tbl = lua.create_table().map_err(|e| anyhow::anyhow!("{e}"))?;
    cframe_tbl
        .set(
            "new",
            lua.create_function(|_, (x, y, z): (Option<f32>, Option<f32>, Option<f32>)| {
                Ok(RbxCFrame(Vec3::new(
                    x.unwrap_or(0.0),
                    y.unwrap_or(0.0),
                    z.unwrap_or(0.0),
                )))
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    cframe_tbl
        .set(
            "Angles",
            lua.create_function(|_, (_rx, _ry, _rz): (Option<f32>, Option<f32>, Option<f32>)| {
                Ok(RbxCFrame(Vec3::ZERO))
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    globals.set("CFrame", cframe_tbl).map_err(|e| anyhow::anyhow!("{e}"))?;

    // Instance.new
    let inst_tbl = lua.create_table().map_err(|e| anyhow::anyhow!("{e}"))?;
    let dm_for_inst = shared_dm.clone();
    inst_tbl
        .set(
            "new",
            lua.create_function(move |_, (class_name, parent): (String, Option<Value>)| {
                let parent_idx = match parent {
                    Some(Value::UserData(ud)) => ud.borrow::<RbxInstance>().ok().map(|p| p.idx),
                    _ => None,
                };
                let mut dm = dm_for_inst.borrow_mut();
                let is_part = matches!(
                    class_name.as_str(),
                    "Part" | "WedgePart" | "CornerWedgePart" | "TrussPart" | "SpawnLocation"
                );
                let part_index = if is_part {
                    dm.add_part(
                        class_name.clone(),
                        Vec3::ZERO,
                        Vec3::new(4.0, 1.2, 2.0),
                        [0.64, 0.64, 0.64],
                        false,
                    );
                    Some(dm.parts.len() - 1)
                } else {
                    None
                };
                let idx = dm.instances.len();
                dm.instances.push(InstanceNode {
                    name: class_name.clone(),
                    class_name,
                    parent: parent_idx,
                    children: Vec::new(),
                    part_index,
                    script_index: None,
                    disabled: false,
                    value_num: 0.0,
                    value_str: String::new(),
                    value_bool: false,
                });
                if let Some(p) = parent_idx {
                    dm.instances[p].children.push(idx);
                }
                Ok(RbxInstance {
                    dm: dm_for_inst.clone(),
                    idx,
                })
            })
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    globals.set("Instance", inst_tbl).map_err(|e| anyhow::anyhow!("{e}"))?;

    // Classic Roblox timing / coroutine helpers
    let wc = wait_calls.clone();
    let wait_fn = lua
        .create_function(move |_, _secs: Option<f64>| {
            let c = wc.get().saturating_add(1);
            wc.set(c);
            if c > 3 {
                Err(mlua::Error::RuntimeError(YIELD_SENTINEL.into()))
            } else {
                Ok(0.03_f64)
            }
        })
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    globals.set("wait", wait_fn.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;
    globals.set("Wait", wait_fn).map_err(|e| anyhow::anyhow!("{e}"))?;

    let noop_fn = lua
        .create_function(|_, _args: mlua::Variadic<Value>| Ok(()))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    for name in ["delay", "Delay", "spawn", "Spawn"] {
        globals.set(name, noop_fn.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    let zero_fn = lua.create_function(|_, ()| Ok(0.0_f64)).map_err(|e| anyhow::anyhow!("{e}"))?;
    globals.set("tick", zero_fn.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;
    globals.set("time", zero_fn).map_err(|e| anyhow::anyhow!("{e}"))?;

    // game / Game / workspace / Workspace
    let game_inst = RbxInstance {
        dm: shared_dm.clone(),
        idx: 0,
    };
    let ws_inst = RbxInstance {
        dm: shared_dm.clone(),
        idx: ws_idx,
    };
    globals.set("game", game_inst.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;
    globals.set("Game", game_inst).map_err(|e| anyhow::anyhow!("{e}"))?;
    globals.set("workspace", ws_inst.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;
    globals.set("Workspace", ws_inst.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut logs = Vec::new();
    for (i, (name, source)) in scripts.iter().enumerate() {
        let script_idx = script_instances.get(i).copied().unwrap_or(ws_idx);
        if shared_dm.borrow().instances[script_idx].disabled {
            continue;
        }
        globals
            .set(
                "script",
                RbxInstance {
                    dm: shared_dm.clone(),
                    idx: script_idx,
                },
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        wait_calls.set(0);
        vm_steps.set(0);

        match lua.load(source).set_name(name).exec() {
            Ok(()) => logs.push(format!("[Luau] executed {name}")),
            Err(err) => {
                let msg = err.to_string();
                if msg.contains(YIELD_SENTINEL) {
                    logs.push(format!("[Luau] executed {name} (yielded)"));
                } else {
                    logs.push(format!("[Luau] error in {name}: {msg}"));
                }
            }
        }
    }

    drop(ws_inst);
    drop(globals);
    let _ = lua.gc_collect();
    drop(lua);
    *dm = std::mem::take(&mut *shared_dm.borrow_mut());
    Ok(logs)
}
