# binarygcode

A `no_std` + `alloc` rust library and binary (CLI) to deserialise and serialise binary gcode (`.bgcode`) files. The binary gcode specification can be found [here](https://github.com/prusa3d/libbgcode/blob/main/doc/specifications.md).

# Support

Please consider supporting the crate by:

- Downloading and using the crate.
- Raising issues and improvements on the GitHub repo.
- Recommending the crate to others.
- ⭐ the crate on GitHub.
- Sponsoring the [maintainer](https://github.com/sponsors/jamesgopsill).

# Example

Examples can be found in the `examples` folder. Below is an example of reading the headers

```rust
use std::{env, fs};

use binarygcode::binary_to_ascii;

fn main() {
    // Create the path to the gcode file
    let mut path = env::current_dir().unwrap();
    path.push("test_files");
    path.push("mini_cube_b.bgcode");

    let binary = fs::read(path).unwrap();
    let gcode = binary_to_ascii(&binary, true).unwrap();
    println!(
        "Binary Length: {}, ASCII Lenght: {}",
        binary.len(),
        gcode.len()
    );

    fs::write("tmp/ascii.gcode", gcode.as_ref()).unwrap();
}
```

# References

- <https://github.com/prusa3d/libbgcode>
