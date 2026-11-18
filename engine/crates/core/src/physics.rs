use glam::Vec3;
use rapier3d::prelude::*;

use crate::DataModel;

pub struct PhysicsWorld {
    pipeline: PhysicsPipeline,
    gravity: Vector<f32>,
    integration: IntegrationParameters,
    islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd_solver: CCDSolver,
    part_bodies: Vec<(u64, RigidBodyHandle)>,
}

impl PhysicsWorld {
    pub fn from_data_model(dm: &DataModel) -> Self {
        let mut bodies = RigidBodySet::new();
        let mut colliders = ColliderSet::new();
        let mut part_bodies = Vec::new();

        for (idx, part) in dm.parts.iter().enumerate() {
            let mat = glam::Mat3::from_cols_array_2d(&part.rotation);
            let q = glam::Quat::from_mat3(&mat);
            let iso = Isometry::from_parts(
                Translation::new(part.position.x, part.position.y, part.position.z),
                rapier3d::na::UnitQuaternion::from_quaternion(rapier3d::na::Quaternion::new(
                    q.w, q.x, q.y, q.z,
                )),
            );

            let rb = if part.anchored {
                RigidBodyBuilder::fixed()
            } else {
                RigidBodyBuilder::dynamic()
            }
            .position(iso)
            .build();

            let handle = bodies.insert(rb);
            let collider = ColliderBuilder::cuboid(
                (part.size.x * 0.5).max(0.05),
                (part.size.y * 0.5).max(0.05),
                (part.size.z * 0.5).max(0.05),
            )
            .build();
            colliders.insert_with_parent(collider, handle, &mut bodies);
            if !part.anchored {
                part_bodies.push((idx as u64, handle));
            }
        }

        Self {
            pipeline: PhysicsPipeline::new(),
            gravity: vector![0.0, -32.0, 0.0],
            integration: IntegrationParameters::default(),
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            bodies,
            colliders,
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            part_bodies,
        }
    }

    pub fn step(&mut self, dt: f32, dm: &mut DataModel) {
        self.integration.dt = dt;
        self.pipeline.step(
            &self.gravity,
            &self.integration,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd_solver,
            None,
            &(),
            &(),
        );

        for (part_idx, handle) in &self.part_bodies {
            if let Some(body) = self.bodies.get(*handle) {
                if let Some(part) = dm.parts.get_mut(*part_idx as usize) {
                    let t = body.translation();
                    part.position = Vec3::new(t.x, t.y, t.z);
                    let r = body.rotation();
                    let q = glam::Quat::from_xyzw(r.i, r.j, r.k, r.w);
                    part.rotation = glam::Mat3::from_quat(q).to_cols_array_2d();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unanchored_part_falls_under_gravity() {
        let mut dm = DataModel::new_baseplate();
        let falling_id = dm.add_part(
            "FallingBox",
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(2.0, 2.0, 2.0),
            [0.2, 0.7, 0.9],
            false,
        );
        let mut world = PhysicsWorld::from_data_model(&dm);
        for _ in 0..30 {
            world.step(1.0 / 60.0, &mut dm);
        }
        let falling = dm.parts.iter().find(|p| p.id == falling_id).unwrap();
        assert!(falling.position.y < 10.0);
    }
}
