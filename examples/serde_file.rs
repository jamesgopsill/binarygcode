use std::{env, fs};

use binarygcode::convert::{ascii_to_binary, binary_to_ascii};

fn main() {
	// Create the path to the gcode file
	let mut path = env::current_dir().unwrap();
	path.push("test_files");
	path.push("mini_cube_b.gcode");

	// Read the data into memory
	let before = fs::read_to_string(path).unwrap();
	println!("Before: {}", before.len());
	// Convert to binary
	let binary = ascii_to_binary(&before).unwrap();
	println!("Binary: {}", binary.len());
	// Convert back to str
	let after = binary_to_ascii(&binary).unwrap();
	println!("After: {}", after.len());

	// Lets see what we get
	fs::write("tmp/serde.gcode", after.as_ref()).unwrap();
}
