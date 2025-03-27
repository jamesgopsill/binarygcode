use std::{
	env,
	fs::File,
	io::{BufReader, Read},
};

use binarygcode::{
	block::{Block, CompressionAlgorithm},
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

	let mut block_header_bytes = [0u8; 12];
	while reader.read_exact(block_header_bytes.as_mut_slice()).is_ok() {
		let block = Block::read_header(&block_header_bytes).unwrap();

		println!("{:?}", block);
		// Must seek back as the header would have been only 8.
		if block.compression == CompressionAlgorithm::None {
			println!("Stepping back");
			reader.seek_relative(-4).unwrap();
		}

		let size = block.block_size(&file_header.checksum);
		println!("Block Size: {}", size);
		let size: i64 = size.try_into().unwrap();
		reader.seek_relative(size).unwrap();
	}
}
