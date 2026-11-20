//! Clean-room 2016 Roblox Network Schema (`ID_SCHEMA_SYNC` / `ID_NEW_SCHEMA`) and `ID_PHYSICS` packets.

use anyhow::{bail, Result};

use crate::{
    bitstream::{BitReader, BitWriter},
    packets::PacketId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDescriptor {
    pub name: String,
    pub bit_size: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyDescriptor {
    pub name: String,
    pub value_type: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDescriptor {
    pub name: String,
    pub properties: Vec<PropertyDescriptor>,
    pub events: Vec<String>,
}

/// Clean-room 2016 reflection/network schema exchanged via `ID_SCHEMA_SYNC` (0x91) / `ID_NEW_SCHEMA` (0x97).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSchema {
    pub schema_version: u32,
    pub enums: Vec<EnumDescriptor>,
    pub classes: Vec<ClassDescriptor>,
}

impl NetworkSchema {
    /// Core 2016 Roblox class & enum schema covering Workspace, Parts, Players, Humanoids, Scripts, and Remotes.
    pub fn default_2016() -> Self {
        Self {
            schema_version: 2016,
            enums: vec![
                EnumDescriptor { name: "PartType".into(), bit_size: 3 },
                EnumDescriptor { name: "Material".into(), bit_size: 6 },
                EnumDescriptor { name: "SurfaceType".into(), bit_size: 4 },
                EnumDescriptor { name: "FormFactor".into(), bit_size: 3 },
                EnumDescriptor { name: "HumanoidStateType".into(), bit_size: 5 },
            ],
            classes: vec![
                ClassDescriptor {
                    name: "Instance".into(),
                    properties: vec![
                        PropertyDescriptor { name: "Name".into(), value_type: 1 },
                        PropertyDescriptor { name: "Parent".into(), value_type: 2 },
                    ],
                    events: vec!["ChildAdded".into(), "ChildRemoved".into()],
                },
                ClassDescriptor {
                    name: "Workspace".into(),
                    properties: vec![
                        PropertyDescriptor { name: "DistributedGameTime".into(), value_type: 5 },
                        PropertyDescriptor { name: "Gravity".into(), value_type: 4 },
                    ],
                    events: vec![],
                },
                ClassDescriptor {
                    name: "Part".into(),
                    properties: vec![
                        PropertyDescriptor { name: "CFrame".into(), value_type: 12 },
                        PropertyDescriptor { name: "Size".into(), value_type: 11 },
                        PropertyDescriptor { name: "BrickColor".into(), value_type: 8 },
                        PropertyDescriptor { name: "Color".into(), value_type: 9 },
                        PropertyDescriptor { name: "Transparency".into(), value_type: 4 },
                        PropertyDescriptor { name: "Anchored".into(), value_type: 3 },
                        PropertyDescriptor { name: "CanCollide".into(), value_type: 3 },
                        PropertyDescriptor { name: "Shape".into(), value_type: 7 },
                        PropertyDescriptor { name: "Material".into(), value_type: 7 },
                    ],
                    events: vec!["Touched".into()],
                },
                ClassDescriptor {
                    name: "Model".into(),
                    properties: vec![PropertyDescriptor { name: "PrimaryPart".into(), value_type: 2 }],
                    events: vec![],
                },
                ClassDescriptor {
                    name: "Players".into(),
                    properties: vec![PropertyDescriptor { name: "MaxPlayers".into(), value_type: 6 }],
                    events: vec!["PlayerAdded".into(), "PlayerRemoving".into()],
                },
                ClassDescriptor {
                    name: "Player".into(),
                    properties: vec![
                        PropertyDescriptor { name: "UserId".into(), value_type: 6 },
                        PropertyDescriptor { name: "Character".into(), value_type: 2 },
                        PropertyDescriptor { name: "CharacterAppearance".into(), value_type: 1 },
                    ],
                    events: vec!["Chatted".into()],
                },
                ClassDescriptor {
                    name: "Humanoid".into(),
                    properties: vec![
                        PropertyDescriptor { name: "Health".into(), value_type: 4 },
                        PropertyDescriptor { name: "MaxHealth".into(), value_type: 4 },
                        PropertyDescriptor { name: "WalkSpeed".into(), value_type: 4 },
                        PropertyDescriptor { name: "JumpPower".into(), value_type: 4 },
                    ],
                    events: vec!["Died".into()],
                },
                ClassDescriptor {
                    name: "RemoteEvent".into(),
                    properties: vec![],
                    events: vec!["OnServerEvent".into(), "OnClientEvent".into()],
                },
                ClassDescriptor {
                    name: "Lighting".into(),
                    properties: vec![
                        PropertyDescriptor { name: "ClockTime".into(), value_type: 4 },
                        PropertyDescriptor { name: "Brightness".into(), value_type: 4 },
                    ],
                    events: vec![],
                },
            ],
        }
    }

    /// Encodes `ID_SCHEMA_SYNC` (0x91) or `ID_NEW_SCHEMA` (0x97) using MSB-first BitStream packing.
    pub fn encode(&self, packet_id: PacketId) -> Vec<u8> {
        let mut bw = BitWriter::with_capacity(512);
        bw.write_u8(packet_id as u8);
        bw.write_varint(self.schema_version);
        bw.write_varint(self.enums.len() as u32);
        for e in &self.enums {
            bw.write_string_varint(&e.name);
            bw.write_bits(e.bit_size as u64, 6);
        }
        bw.align_byte();
        bw.write_varint(self.classes.len() as u32);
        for c in &self.classes {
            bw.write_string_varint(&c.name);
            bw.write_varint(c.properties.len() as u32);
            for p in &c.properties {
                bw.write_string_varint(&p.name);
                bw.write_u8(p.value_type);
            }
            bw.write_varint(c.events.len() as u32);
            for ev in &c.events {
                bw.write_string_varint(ev);
            }
        }
        bw.into_bytes()
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut br = BitReader::new(buf);
        let id = br.read_u8()?;
        if id != PacketId::SchemaSync as u8 && id != PacketId::NewSchema as u8 {
            bail!("expected ID_SCHEMA_SYNC (0x91) or ID_NEW_SCHEMA (0x97), got 0x{id:02X}");
        }
        let schema_version = br.read_varint()?;
        let enum_count = br.read_varint()? as usize;
        let mut enums = Vec::with_capacity(enum_count);
        for _ in 0..enum_count {
            let name = br.read_string_varint()?;
            let bit_size = br.read_bits(6)? as u8;
            enums.push(EnumDescriptor { name, bit_size });
        }
        br.align_byte();
        let class_count = br.read_varint()? as usize;
        let mut classes = Vec::with_capacity(class_count);
        for _ in 0..class_count {
            let name = br.read_string_varint()?;
            let prop_count = br.read_varint()? as usize;
            let mut properties = Vec::with_capacity(prop_count);
            for _ in 0..prop_count {
                let pname = br.read_string_varint()?;
                let value_type = br.read_u8()?;
                properties.push(PropertyDescriptor {
                    name: pname,
                    value_type,
                });
            }
            let ev_count = br.read_varint()? as usize;
            let mut events = Vec::with_capacity(ev_count);
            for _ in 0..ev_count {
                events.push(br.read_string_varint()?);
            }
            classes.push(ClassDescriptor {
                name,
                properties,
                events,
            });
        }
        Ok(Self {
            schema_version,
            enums,
            classes,
        })
    }
}

/// One replicated physics assembly inside `ID_PHYSICS` (0x85).
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicsPartState {
    pub referent_id: u32,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub humanoid_state: u8,
}

/// `ID_PHYSICS` (0x85) packet containing one or more physics assembly snapshots.
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicsPacket {
    pub parts: Vec<PhysicsPartState>,
}

impl PhysicsPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut bw = BitWriter::with_capacity(16 + self.parts.len() * 44);
        bw.write_u8(PacketId::Physics as u8);
        bw.write_varint(self.parts.len() as u32);
        for p in &self.parts {
            bw.write_varint(p.referent_id);
            bw.write_bits(p.humanoid_state as u64, 5);
            bw.align_byte();
            for f in p.position {
                bw.write_f32_be(f);
            }
            for f in p.velocity {
                bw.write_f32_be(f);
            }
            for f in p.angular_velocity {
                bw.write_f32_be(f);
            }
        }
        bw.into_bytes()
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut br = BitReader::new(buf);
        let id = br.read_u8()?;
        if id != PacketId::Physics as u8 {
            bail!("expected ID_PHYSICS (0x85), got 0x{id:02X}");
        }
        let count = br.read_varint()? as usize;
        if count > 4096 {
            bail!("ID_PHYSICS part count too large: {count}");
        }
        let mut parts = Vec::with_capacity(count);
        for _ in 0..count {
            let referent_id = br.read_varint()?;
            let humanoid_state = br.read_bits(5)? as u8;
            br.align_byte();
            let position = [br.read_f32_be()?, br.read_f32_be()?, br.read_f32_be()?];
            let velocity = [br.read_f32_be()?, br.read_f32_be()?, br.read_f32_be()?];
            let angular_velocity = [br.read_f32_be()?, br.read_f32_be()?, br.read_f32_be()?];
            parts.push(PhysicsPartState {
                referent_id,
                position,
                velocity,
                angular_velocity,
                humanoid_state,
            });
        }
        Ok(Self { parts })
    }
}
