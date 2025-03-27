use core::str;
use std::{
	env,
	fs::File,
	io::{BufReader, Read},
};

use binarygcode::{
	block::{BlockDeserialiser, BlockKind},
	file_header::FileHeader,
};

fn main() {
	let mut path = env::current_dir().unwrap();
	path.push("test_files");
	path.push("mini_cube_b.bgcode");
	let file = File::open(path).unwrap();
	let mut reader = BufReader::new(file);

	let mut file_header_bytes = [0u8; 10];
	reader.read_exact(file_header_bytes.as_mut_slice()).unwrap();
	let file_header = FileHeader::from_bytes(&file_header_bytes).unwrap();
	println!("{:?}", file_header);

	let mut block = BlockDeserialiser::new(file_header.checksum);
	while reader.read_exact(block.header_buf()).is_ok() {
		println!("{:?}", block);

		let data_buf = block.data_buf().unwrap();
		reader.read_exact(data_buf).unwrap();
		println!(
			"{:?} {}",
			block.kind().unwrap(),
			block.block_size().unwrap()
		);

		let data = block.deserialise().unwrap();
		match block.kind().unwrap() {
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
			_ => {
				println!("Data Length: {}", data.len());
			}
		}
	}

	//let mut block_header_bytes = [0u8; 12];
	//while reader.read_exact(block_header_bytes.as_mut_slice()).is_ok() {

	/*
	println!("{:?}", block);
	// Must seek back as the header would have been only 8.
	if block.compression == CompressionAlgorithm::None {
		println!("Stepping back");
		reader.seek_relative(-4).unwrap();
	}

	let size = block.block_size(&file_header.checksum);
	let mut buf = vec![0; size];
	println!("{:?}", buf);
	reader.read_exact(&mut buf).unwrap();
	let data = block
		.deserialise_block_data(buf.as_slice(), &file_header.checksum)
		.unwrap();
	println!("{:?}", data);
	match block.kind {
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
		_ => {}
	}
	*/
	//}
}
