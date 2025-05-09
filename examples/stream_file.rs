use std::{
    env,
    fs::File,
    io::{BufReader, Read},
};

use binarygcode::{DeserialisedResult, Deserialiser};

fn main() {
    // Create the path to the gcode file
    let mut path = env::current_dir().unwrap();
    path.push("test_files");
    //path.push("mini_cube_b.bgcode");
    path.push("mini_cube_ps2.8.1.bgcode");

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
