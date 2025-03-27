# binarygcode

A `no_std` + `alloc` compatible rust library and binary crate providing parsing of binary gcode files.

# Example

``` rust
use std::{
	env,
	fs::File,
	io::{BufReader, Read},
};

use binarygcode::file_header::FileHeader;

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
}
```
