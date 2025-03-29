use alloc::{boxed::Box, vec::Vec};
use embedded_heatshrink::{HSDPollRes, HSEPollRes, HeatshrinkEncoder};
use miniz_oxide::deflate::compress_to_vec_zlib;

use crate::common::{
	BinaryGcodeChecksum, BinaryGcodeError, BlockKind, CompressionAlgorithm, Encoding,
};

pub struct BlockSerialiser {
	pub kind: BlockKind,
	pub compression: CompressionAlgorithm,
	pub checksum: BinaryGcodeChecksum,
	pub encoding: Encoding,
}

impl BlockSerialiser {
	pub fn new(
		kind: BlockKind,
		compression: CompressionAlgorithm,
		checksum: BinaryGcodeChecksum,
		encoding: Encoding,
	) -> Result<Self, BinaryGcodeError> {
		let s = Self {
			kind,
			compression,
			checksum,
			encoding,
		};
		s.validate_config()?;
		Ok(s)
	}

	fn validate_config(&self) -> Result<(), BinaryGcodeError> {
		// TODO
		Ok(())
	}

	// Expects the string to have already been encoded
	pub fn serialise(
		&self,
		input: &[u8],
	) -> Result<Box<[u8]>, BinaryGcodeError> {
		self.validate_config()?;

		let mut out: Vec<u8> = Vec::new();

		// Write out the header.
		out.extend(self.kind.to_le_bytes());
		out.extend(self.compression.to_le_bytes());
		let unc_size = input.len() as u32;
		out.extend(unc_size.to_le_bytes());

		// Compress the data
		let data: Vec<u8> = Vec::new();
		match self.compression {
			CompressionAlgorithm::None => {
				// TODO:  Add the parameters

				//
				out.extend(input);
			}
			CompressionAlgorithm::Deflate => {
				let c = compress_to_vec_zlib(input, 10); // TODO: check compression matches
				let c_size = c.len() as u32;
				out.extend(c_size.to_le_bytes());
				out.extend(self.encoding.to_le_bytes());
				out.extend(c);
			}
			CompressionAlgorithm::Heatshrink11_4 => {
				let c = self.heatshrink(input, 11, 4)?;
				let c_size = c.len() as u32;
				out.extend(c_size.to_le_bytes());
				out.extend(self.encoding.to_le_bytes());
				out.extend(c);
			}
			CompressionAlgorithm::Heatshrink12_4 => {
				let c = self.heatshrink(input, 12, 4)?;
				let c_size = c.len() as u32;
				out.extend(c_size.to_le_bytes());
				out.extend(self.encoding.to_le_bytes());
				out.extend(c);
			}
		}

		// Append the checksum

		Ok(out.into_boxed_slice())
	}

	fn heatshrink(
		&self,
		input: &[u8],
		window: u8,
		lookahead: u8,
	) -> Result<Vec<u8>, BinaryGcodeError> {
		let mut encoder = HeatshrinkEncoder::new(window, lookahead).unwrap();
		let mut out: Vec<u8> = vec![0; input.len()];
		let res = encoder.sink_all(input, &mut out);
		match res {
			HSEPollRes::Empty(count) => Ok(out[0..count].to_vec()),
			_ => Err(BinaryGcodeError::HeatshrinkError),
		}
	}
}
