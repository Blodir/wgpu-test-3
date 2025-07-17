use gltf::Gltf;
use std::env;

fn generate_mesh_primitive() {}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let path = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("assets/Lantern.glb");
    let gltf = Gltf::open(path)?;
    for scene in gltf.scenes() {
        for node in scene.nodes() {
            println!(
                "Node #{} has {} children",
                node.index(),
                node.children().count(),
            );
            for primitive in node.mesh().unwrap().primitives() {}
        }
    }

    Ok(())
}
