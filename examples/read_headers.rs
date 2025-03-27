use std::{
	env,
	fs::File,
	io::{BufReader, Read},
};

use binarygcode::{deserialiser::BlockDeserialiser, header::FileHeader};

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
	}
}
