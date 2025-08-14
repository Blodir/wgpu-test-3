use std::env;
use std::fs::File;
use std::io;

use wgpu_test_3::run;

fn main() -> io::Result<()> {
    /*
    let args: Vec<String> = env::args().collect();
    let path = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("BoxInterleaved.glb");
    let mut file = File::open(path)?;
    */

    run();

    Ok(())
}
