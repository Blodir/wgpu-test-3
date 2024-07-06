use std::fs::File;
use std::io::{self, Read};

mod renderer;
mod app;
mod glb;

fn main() -> io::Result<()> {
    let path = "KX.1325.glb";
    let mut file = File::open(path)?;

    let glb = glb::GLBObject::new(&mut file);

    match glb {
        Ok(mesh) => {
            println!("magic: {}, version: {}, length: {}", mesh.magic, mesh.version, mesh.length);
            println!("JSON CHUNK | length: {}, type: {}, data:", mesh.json_chunk.chunk_length, mesh.json_chunk.chunk_type);
            let json_value: serde_json::Value = serde_json::from_str(&mesh.json_chunk.raw_json)?;
            let pretty_json = serde_json::to_string_pretty(&json_value)?;
            println!("{}", pretty_json);
            println!("Parsed JSON: {:#?}", mesh.json_chunk.chunk_data);
        },
        Err(e) => {
            eprintln!("Failed to read file: {}", e);
        }
    };

    //app::run();

    Ok(())
}
