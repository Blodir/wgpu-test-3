use std::env;
use std::fs::File;
use std::io;

use wgpu_test_3::run;

fn main() -> io::Result<()> {
    run();
    Ok(())
}
