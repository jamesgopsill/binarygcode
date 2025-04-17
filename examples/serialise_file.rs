use std::{env, fs};

use binarygcode::convert::ascii_to_binary;

fn main() {
	// Create the path to the gcode file
	let mut path = env::current_dir().unwrap();
	path.push("test_files");
	path.push("mini_cube_b.gcode");

	let gcode = fs::read_to_string(path).unwrap();
	let binary = ascii_to_binary(&gcode).unwrap();
	println!("gcode: {}, binary: {}", gcode.len(), binary.len());

	fs::write("tmp/mini_cube_b.bgcode", binary.as_ref()).unwrap();
}
