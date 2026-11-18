use std::path::PathBuf;

use revival_client::{renderer::render_offscreen_png, ClientApp, ClientSession};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--screenshot") {
        let out_path = PathBuf::from(
            args.get(pos + 1)
                .map(String::as_str)
                .unwrap_or("screenshot.png"),
        );
        let mut session = if let Some(place_path) = args.get(pos + 2) {
            let bytes = std::fs::read(place_path)?;
            println!("loading place {place_path}...");
            ClientSession::from_place_bytes(&bytes)?
        } else {
            let mut s = ClientSession::offline_baseplate();
            s.data_model.add_part(
                "BlueArch",
                glam::Vec3::new(-6.0, 2.0, -6.0),
                glam::Vec3::new(6.0, 4.0, 2.0),
                [0.2, 0.45, 0.9],
                true,
            );
            s.data_model.add_part(
                "YellowTower",
                glam::Vec3::new(8.0, 4.0, -8.0),
                glam::Vec3::new(3.0, 8.0, 3.0),
                [0.95, 0.8, 0.15],
                true,
            );
            s
        };
        for line in session.data_model.run_luau_scripts_mut()? {
            println!("{line}");
        }
        println!(
            "rendering {} parts (skybox_up={:?}, sun={:?})...",
            session.data_model.parts.len(),
            session.data_model.sky.skybox_up,
            session.data_model.sky.sun_direction
        );
        render_offscreen_png(
            &session.data_model.parts,
            &session.data_model.sky,
            960,
            540,
            &out_path,
        )?;
        println!("rendered real engine screenshot to {}", out_path.display());
        return Ok(());
    }

    let mut session = if let Some(path) = args.get(1) {
        let bytes = std::fs::read(path)?;
        ClientSession::from_place_bytes(&bytes)?
    } else {
        ClientSession::offline_baseplate()
    };

    for line in session.data_model.run_luau_scripts_mut()? {
        println!("{line}");
    }
    session.physics = revival_core::physics::PhysicsWorld::from_data_model(&session.data_model);

    ClientApp::new(session).run()
}
