# binarygcode

A `no_std` + `alloc` rust library and (coming soon) binary crate to deserialise and serialise binary gcode (`.bgcode`) files. The binary gcode specification can be found [here](https://github.com/prusa3d/libbgcode/blob/main/doc/specifications.md).

# Support

Please consider supporting the crate by:

- Downloading and using the crate.
- Raising issues and improvements on the GitHub repo.
- Recommending the crate to others.
- ⭐ the crate on GitHub.
- Sponsoring the [maintainer](https://github.com/sponsors/jamesgopsill).


# Functionality

The crate is still under construction. So far we have managed to complete...

| **Function** | **Status** |
| --- | --- |
| Deserialise | Done |
| Serialise | In Progress |
| Binary (CLI) | Planned |

# Example

Examples can be found in the `examples` folder. Below is an example of reading the headers

```rust
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
				}
				DeserialisedResult::MoreBytesRequired(_) => {
					break;
				}
			}
		}
	}
}
```

# References

- <https://github.com/prusa3d/libbgcode>
