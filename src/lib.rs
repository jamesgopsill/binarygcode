#![doc = include_str!("../README.md")]
#![no_std]

#[macro_use]
extern crate alloc;

mod components;

pub use components::common::{BinaryGcodeError, BlockKind, Checksum};
pub use components::convert::{ascii_to_binary, binary_to_ascii};
pub use components::deserialiser::{
    DeserialisedBlock, DeserialisedFileHeader, DeserialisedResult, Deserialiser,
};
pub use components::serialiser::{serialise_block, serialise_file_header};
