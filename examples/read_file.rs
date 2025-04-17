use std::{env, fs};

use binarygcode::convert::binary_to_ascii;

fn main() {
	// Create the path to the gcode file
	let mut path = env::current_dir().unwrap();
	path.push("test_files");
	path.push("mini_cube_b.bgcode");

	let binary = fs::read(path).unwrap();
	let gcode = binary_to_ascii(&binary).unwrap();
	println!("Length: {}", gcode.len());

	fs::write("tmp/ascii.gcode", gcode.as_ref()).unwrap();
}
