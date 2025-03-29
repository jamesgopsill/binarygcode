use core::str;
use std::{
	env,
	fs::File,
	io::{BufReader, Read},
};

use binarygcode::{
	common::{BlockKind, Encoding},
	deserialiser::Deserialiser,
};
use meatpack::{MeatPackResult, Unpacker};

fn main() {
	// Create the path to the gcode file
	let mut path = env::current_dir().unwrap();
	path.push("test_files");
	path.push("mini_cube_b.bgcode");

	// Open the file and attach a reader
	let file = File::open(path).unwrap();
	let mut reader = BufReader::new(file);

	// Read the file header bytes.
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
	while reader.read_exact(deserialiser.block_header_buf()).is_ok() {
		println!(
			"## New Block ##\n{:?} {}",
			deserialiser.kind().unwrap(),
			deserialiser.block_size().unwrap()
		);

		// Get a buffer of the right size for the data
		// expected by the block and fill it with the mechanism
		// you're retrieving you data from. For example, a file I/O or
		// data stream.
		let data_buf = deserialiser.block_data_buf().unwrap();
		reader.read_exact(data_buf).unwrap();

		let data = deserialiser.deserialise().unwrap();

		// Decide what you want to do with the data.
		match deserialiser.kind().unwrap() {
			BlockKind::FileMetadata => {
				let s = str::from_utf8(&data).unwrap();
				println!("{:?}", s);
			}
			BlockKind::PrinterMetadata => {
				let s = str::from_utf8(&data).unwrap();
				println!("{:?}", s);
			}
			BlockKind::PrintMetadata => {
				let s = str::from_utf8(&data).unwrap();
				println!("{:?}", s);
			}
			BlockKind::GCode => match deserialiser.encoding().unwrap() {
				Encoding::ASCII => {
					let s = str::from_utf8(&data).unwrap();
					println!("{:?}", s);
				}
				Encoding::Meatpack => {
					// Use the Meatpack crate to re-encode back to ASCII Gcode.
					let mut unpacker = Unpacker::<64>::default();
					for byte in data {
						let res = unpacker.unpack(&byte);
						match res {
							Ok(MeatPackResult::WaitingForNextByte) => {}
							Ok(MeatPackResult::Line(line)) => {
								let s = str::from_utf8(line).unwrap();
								println!("{:?}", s);
							}
							Err(e) => {
								println!("{:?}", e);
							}
						}
					}
				}
				Encoding::MeatpackWithComments => {
					let mut unpacker = Unpacker::<64>::default();
					for byte in data {
						let res = unpacker.unpack(&byte);
						match res {
							Ok(MeatPackResult::WaitingForNextByte) => {}
							Ok(MeatPackResult::Line(line)) => {
								let s = str::from_utf8(line).unwrap();
								println!("{:?}", s);
							}
							Err(e) => {
								println!("{:?}", e);
							}
						}
					}
				}
				_ => {}
			},
			_ => {
				println!("Data Length: {}", data.len());
			}
		}
	}
}
