use std::env;
use std::fs::File;
use std::io::{self, Read};

use glb::GLTFSceneRef;

mod wgpu_context;
mod camera;
mod renderer;
mod app;
mod glb;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    //let path = "KX.1325.glb";
    //let path = "BoxInterleaved.glb";
    //let path = "Duck.glb";
    let path = args.get(1).map(String::as_str).unwrap_or("BoxInterleaved.glb");
    let mut file = File::open(path)?;

    let glb = glb::GLBObject::new(&mut file);

    match glb {
        Ok(glb_data) => {
            println!("magic: {}, version: {}, length: {}", glb_data.magic, glb_data.version, glb_data.length);
            println!("JSON CHUNK | length: {}, type: {}, data:", glb_data.json_chunk.chunk_length, glb_data.json_chunk.chunk_type);
            let json_value: serde_json::Value = serde_json::from_str(&glb_data.json_chunk.raw_json)?;
            let pretty_json = serde_json::to_string_pretty(&json_value)?;
            println!("{}", pretty_json);
            println!("Parsed JSON: {:#?}", glb_data.json_chunk.chunk_data);
            println!("binary buffer len: {}", glb_data.binary_buffer.len());
            //println!("Binary data: {}", glb_data.accessor_data_buffers.len());
            let scene_ref = GLTFSceneRef::new(&glb_data);
            app::run(scene_ref);
        },
        Err(e) => {
            eprintln!("Failed to read file: {}", e);
        }
    };

    //app::run();

    Ok(())
}
