use std::{
	env,
	fs::File,
	io::{BufReader, Read},
	str,
};

use binarygcode::{
	common::{BlockKind, Encoding},
	deserialiser::{DeserialisedBlock, DeserialisedResult, Deserialiser},
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

	// Initialise the deserialiser
	let mut deserialiser = Deserialiser::default();

	// Initialise the read buffer. This could be reading from a file
	// or waiting for intermittent bytes from a network transfer.
	let mut buf = [0u8; 256];

	loop {
		// Read bytes into the buffer
		let read = reader.read(buf.as_mut_slice()).unwrap();
		// Exit when exhausted
		if read == 0 {
			break;
		}
		// Provide the read bytes to the deserialiser
		deserialiser.digest(&buf[..read]);

		// Loop through running deserialise on the deserialisers inner
		// buffer with it returning either a header, block or request for more bytes.
		// Or an error when deserialising.
		loop {
			let r = deserialiser.deserialise().unwrap();
			match r {
				DeserialisedResult::FileHeader(fh) => {
					println!("{:?}", fh);
				}
				DeserialisedResult::Block(b) => {
					println!("{}", b);
					print_block_contents(&b);
				}
				DeserialisedResult::MoreBytesRequired(_) => {
					break;
				}
			}
		}
	}
}

// Prints the contents of the block
fn print_block_contents(b: &DeserialisedBlock) {
	let data = b.decompress().unwrap();
	match b.kind {
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
		BlockKind::GCode => match b.encoding {
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
