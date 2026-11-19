use std::path::PathBuf;

use revival_client::{renderer::render_offscreen_png, CharacterState, ClientApp, ClientSession};

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
            ClientSession::offline_baseplate()
        };
        for line in session.data_model.run_luau_scripts_mut()? {
            println!("{line}");
        }
        let character = CharacterState::spawn(&mut session.data_model);
        if !session.data_model.tools.is_empty() {
            character.sync_equipped_tool(0, &mut session.data_model);
        }
        println!(
            "rendering {} parts, {} tools (skybox_up={:?}, sun={:?})...",
            session.data_model.parts.len(),
            session.data_model.tools.len(),
            session.data_model.sky.skybox_up,
            session.data_model.sky.sun_direction
        );
        render_offscreen_png(
            &session.data_model.parts,
            &session.data_model.sky,
            &session.data_model.tools,
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
