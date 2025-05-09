use alloc::{boxed::Box, vec::Vec};
use embedded_heatshrink::{
    HSEFinishRes, HSEPollRes, HSESinkRes, HeatshrinkEncoder,
};
use miniz_oxide::deflate::compress_to_vec_zlib;

use crate::components::common::{
    crc32, BinaryGcodeError, BlockKind, Checksum, CompressionAlgorithm,
    Encoding, MAGIC,
};

pub fn serialise_file_header(
    version: u32,
    checksum: Checksum,
) -> Box<[u8]> {
    let mut header = Vec::with_capacity(10);
    header.extend(MAGIC.to_le_bytes());
    header.extend(version.to_le_bytes());
    header.extend(checksum.to_le_bytes());
    header.into_boxed_slice()
}

/// Serialise a gcode block.
pub fn serialise_block(
    kind: BlockKind,
    compression: CompressionAlgorithm,
    encoding: Encoding,
    checksum: Checksum,
    additional_parameters: &[u8],
    data: &[u8],
) -> Result<Box<[u8]>, BinaryGcodeError> {
    // Create the block header
    let mut block: Vec<u8> = Vec::new();
    block.extend(kind.to_le_bytes());
    block.extend(compression.to_le_bytes());
    let uncompressed_len = data.len() as u32;
    block.extend(uncompressed_len.to_le_bytes());

    // Additional parameters beyond encoding
    let mut parameters: Vec<u8> = Vec::with_capacity(0);
    parameters.extend(encoding.to_le_bytes());
    parameters.extend(additional_parameters);
    // We do not append it to the block here as we need to check
    // if the data is going to be compressed.

    // Compression
    match compression {
        CompressionAlgorithm::None => {
            block.extend(parameters);
            block.extend(data);
        }
        CompressionAlgorithm::Deflate => {
            let compressed = compress_to_vec_zlib(data, 10);
            let compressed_len = compressed.len() as u32;
            block.extend(compressed_len.to_le_bytes());
            block.extend(parameters);
            block.extend(compressed);
        }
        CompressionAlgorithm::Heatshrink11_4 => {
            let compressed = shrink(11, 4, data)?;
            let compressed_len = compressed.len() as u32;
            block.extend(compressed_len.to_le_bytes());
            block.extend(parameters);
            block.extend(compressed);
        }
        CompressionAlgorithm::Heatshrink12_4 => {
            let compressed = shrink(12, 4, data)?;
            let compressed_len = compressed.len() as u32;
            block.extend(compressed_len.to_le_bytes());
            block.extend(parameters);
            block.extend(compressed);
        }
    }

    // CRC
    if checksum == Checksum::Crc32 {
        let crc = crc32(&block);
        block.extend(crc.to_le_bytes());
    }

    Ok(block.into_boxed_slice())
}

/// A wrapper around the heatshrink algorithm that can be
/// used to compress gcode.
/// TODO: add a check to limit the size of the input slice.
/// Ask the bgcode spec makers if there is a limit.
fn shrink(
    window: u8,
    lookahead: u8,
    input: &[u8],
) -> Result<Box<[u8]>, BinaryGcodeError> {
    let mut encoder = HeatshrinkEncoder::new(window, lookahead).unwrap();
    let mut sunk: usize = 0;
    let mut polled: usize = 0;

    // Should never be as big as the input
    let mut output = vec![0u8; input.len()];

    // Keep looping until we have sunk all the input data
    while sunk < input.len() {
        // Sink the next
        match encoder.sink(&input[sunk..]) {
            HSESinkRes::Ok(sz) => {
                sunk += sz;
            }
            _ => return Err(BinaryGcodeError::SerialiseError("heatshrink_01")),
        }
        // Loop to get the data out of the encoder.
        loop {
            match encoder.poll(&mut output[polled..]) {
                // Through my trials. Only this is ever called
                // in our scenario.
                HSEPollRes::Empty(sz) => {
                    polled += sz;
                    if sz == 0 {
                        break;
                    }
                }
                _ => {
                    return Err(BinaryGcodeError::SerialiseError(
                        "heatshrink_02",
                    ))
                }
            }
        }
    }

    // Loop the check if there is any data remaining.
    loop {
        match encoder.finish() {
            HSEFinishRes::Done => break,
            HSEFinishRes::More => match encoder.poll(&mut output[polled..]) {
                // Through my trials. Only this was ever called
                // in our scenario.
                HSEPollRes::Empty(sz) => polled += sz,
                _ => {
                    return Err(BinaryGcodeError::SerialiseError(
                        "heatshrink_03",
                    ))
                }
            },
            _ => return Err(BinaryGcodeError::SerialiseError("heatshrink_04")),
        }
    }

    // Resize so we don't pass any null bytes back out.
    output.resize(polled, 0u8);
    Ok(output.into_boxed_slice())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        Checksum, {DeserialisedResult, Deserialiser},
    };

    #[test]
    pub fn serde_gcode_none() {
        let header = serialise_file_header(1, Checksum::Crc32);
        let gcode = "M73 P0 R30";
        let block = serialise_block(
            BlockKind::GCode,
            CompressionAlgorithm::None,
            Encoding::Ascii,
            Checksum::Crc32,
            &[],
            gcode.as_bytes(),
        )
        .unwrap();
        let mut deserialiser = Deserialiser::default();
        deserialiser.digest(&header);
        deserialiser.digest(&block);
        loop {
            let r = deserialiser.deserialise().unwrap();
            match r {
                DeserialisedResult::FileHeader(_) => {}
                DeserialisedResult::Block(_) => {}
                DeserialisedResult::MoreBytesRequired(_) => {
                    break;
                }
            }
        }
    }

    #[test]
    pub fn serde_gcode_deflate() {
        let header = serialise_file_header(1, Checksum::Crc32);
        let gcode = "M73 P0 R30";
        let block = serialise_block(
            BlockKind::GCode,
            CompressionAlgorithm::Deflate,
            Encoding::Ascii,
            Checksum::Crc32,
            &[],
            gcode.as_bytes(),
        )
        .unwrap();
        let mut deserialiser = Deserialiser::default();
        deserialiser.digest(&header);
        deserialiser.digest(&block);
        loop {
            let r = deserialiser.deserialise().unwrap();
            match r {
                DeserialisedResult::FileHeader(_) => {}
                DeserialisedResult::Block(_) => {}
                DeserialisedResult::MoreBytesRequired(_) => {
                    break;
                }
            }
        }
    }

    #[test]
    pub fn serde_gcode_deflate_no_crc() {
        let header = serialise_file_header(1, Checksum::None);
        let gcode = "M73 P0 R30";
        let block = serialise_block(
            BlockKind::GCode,
            CompressionAlgorithm::Deflate,
            Encoding::Ascii,
            Checksum::None,
            &[],
            gcode.as_bytes(),
        )
        .unwrap();
        let mut deserialiser = Deserialiser::default();
        deserialiser.digest(&header);
        deserialiser.digest(&block);
        loop {
            let r = deserialiser.deserialise().unwrap();
            match r {
                DeserialisedResult::FileHeader(_) => {}
                DeserialisedResult::Block(_) => {}
                DeserialisedResult::MoreBytesRequired(_) => {
                    break;
                }
            }
        }
    }
}
