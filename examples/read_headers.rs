use std::{
	env,
	fs::File,
	io::{BufReader, Read},
};

use binarygcode::deserialiser::Deserialiser;

fn main() {
	// Create the path to the gcode file
	let mut path = env::current_dir().unwrap();
	path.push("test_files");
	path.push("mini_cube_b.bgcode");

	// Open the file and attach a reader
	let file = File::open(path).unwrap();
	let mut reader = BufReader::new(file);

	// Initialise the deserialiser by reading in the file header.
	let mut fh_buf = Deserialiser::fh_buf();
	reader.read_exact(fh_buf.as_mut_slice()).unwrap();
	let mut deserialiser = Deserialiser::new(&fh_buf).unwrap();
	println!(
		"File Version: {}, Checksum: {:?}",
		deserialiser.version, deserialiser.checksum
	);

	// Read each block into the deserialisers internal buf.
	// Processing one at a time. Each loop removes the previous
	// block from the internal buffer and adds the next one
	// until we reach EOF.
	let mut n = 0;
	while reader.read_exact(deserialiser.block_header_buf()).is_ok() {
		println!(
			"{} {:?} {}",
			n,
			deserialiser.kind().unwrap(),
			deserialiser.block_size().unwrap()
		);
		let block_size = deserialiser.block_size().unwrap();
		reader.seek_relative(block_size as i64).unwrap();
		n += 1;
	}
}
