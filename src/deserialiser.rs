use core::{array::TryFromSliceError, fmt};

use alloc::{borrow::ToOwned, boxed::Box, vec::Vec};
use embedded_heatshrink::{HSDPollRes, HeatshrinkDecoder};
use miniz_oxide::inflate::decompress_to_vec_zlib;

use crate::common::{
	crc32, BinaryGcodeError, BlockKind, Checksum, CompressionAlgorithm, Encoding, MAGIC,
};

/// A utility enum to keep track of the state of the deserialiser
/// instance when digesting some bytes.
enum DeserialiserState {
	FileHeader,
	Block,
}

/// The possible outputs from a call to deserialise().
#[derive(Debug)]
pub enum DeserialisedResult {
	FileHeader(DeserialisedFileHeader),
	MoreBytesRequired(usize),
	Block(DeserialisedBlock),
}

/// A struct containing the header details of the bgcode.
#[derive(Debug)]
pub struct DeserialisedFileHeader {
	pub magic: u32,
	pub version: u32,
	pub checksum: Checksum,
}

/// A utility function to take a generic slice and return a
/// slice of a specific size.
pub(crate) fn try_from_slice<const N: usize>(buf: &[u8]) -> Result<[u8; N], BinaryGcodeError> {
	let bytes: Result<[u8; N], TryFromSliceError> = buf.try_into();
	match bytes {
		Ok(bytes) => Ok(bytes),
		Err(_) => Err(BinaryGcodeError::TryFromSliceError),
	}
}

/// A binarygcode deserialiser that can parse a bgcode file. It can
/// digest data in chunks and returns header and blocks when available.
/// The block remain compressed so the user can decide which ones they
/// which to decompress.
pub struct Deserialiser {
	pub inner: Vec<u8>,
	state: DeserialiserState,
	checksum: Checksum,
}

impl Default for Deserialiser {
	fn default() -> Self {
		Self {
			inner: Vec::new(),
			state: DeserialiserState::FileHeader,
			checksum: Checksum::None,
		}
	}
}

impl Deserialiser {
	/// Provide some more bytes for the deserialiser to process/
	pub fn digest(
		&mut self,
		buf: &[u8],
	) {
		self.inner.extend(buf);
	}

	/// Reset the deserialisor to its default state.
	pub fn reset(&mut self) {
		self.inner.clear();
		self.state = DeserialiserState::FileHeader;
	}

	/// Try and deserialised either a file header or block element from the
	/// current digest.
	pub fn deserialise(&mut self) -> Result<DeserialisedResult, BinaryGcodeError> {
		match self.state {
			DeserialiserState::FileHeader => self.deserialise_file_header(),
			DeserialiserState::Block => self.deserialise_block(),
		}
	}

	/// An internal function to deserialise the file header.
	fn deserialise_file_header(&mut self) -> Result<DeserialisedResult, BinaryGcodeError> {
		if self.inner.len() < 10 {
			return Ok(DeserialisedResult::MoreBytesRequired(10 - self.inner.len()));
		}
		// We have enough data to read the file header
		let bytes = try_from_slice::<4>(&self.inner[0..=3])?;
		let magic = u32::from_le_bytes(bytes);
		if magic != MAGIC {
			return Err(BinaryGcodeError::InvalidMagic);
		}

		let bytes = try_from_slice::<4>(&self.inner[4..=7])?;
		let version = u32::from_le_bytes(bytes);

		let bytes = try_from_slice::<2>(&self.inner[8..=9])?;
		let checksum_value = u16::from_le_bytes(bytes);

		let checksum = match checksum_value {
			1 => Checksum::Crc32,
			0 => Checksum::None,
			v => return Err(BinaryGcodeError::InvalidChecksumType(v)),
		};

		let fh = DeserialisedFileHeader {
			magic,
			version,
			checksum: checksum.clone(),
		};

		self.checksum = checksum;
		self.state = DeserialiserState::Block;
		self.inner.drain(..10);

		Ok(DeserialisedResult::FileHeader(fh))
	}

	/// An internal function to deserialise a block from the digest.
	fn deserialise_block(&mut self) -> Result<DeserialisedResult, BinaryGcodeError> {
		// Check if we have enough data to read the block header
		if self.inner.len() < 12 {
			return Ok(DeserialisedResult::MoreBytesRequired(12 - self.inner.len()));
		}

		// Check the header
		let bytes = try_from_slice::<2>(&self.inner[0..=1])?;
		let kind = BlockKind::from_le_bytes(bytes)?;

		let bytes = try_from_slice::<2>(&self.inner[2..=3])?;
		let compression = CompressionAlgorithm::from_le_bytes(bytes)?;

		let bytes = try_from_slice::<4>(&self.inner[4..=7])?;
		let data_uncompressed_len = u32::from_le_bytes(bytes) as usize;

		let data_compressed_len: Option<usize> = match compression {
			CompressionAlgorithm::None => None,
			_ => {
				let bytes = try_from_slice::<4>(&self.inner[8..=11])?;
				Some(u32::from_le_bytes(bytes) as usize)
			}
		};

		let param_len = match kind {
			BlockKind::Thumbnail => 6,
			_ => 2,
		};

		// Have we collected all the data we need?
		let block_len = match data_compressed_len {
			Some(cl) => {
				// header + parameters + comrpessed_len
				let mut block_len = 12 + param_len + cl;
				if self.checksum == Checksum::Crc32 {
					block_len += 4;
				}
				if self.inner.len() < block_len {
					return Ok(DeserialisedResult::MoreBytesRequired(
						block_len - self.inner.len(),
					));
				}
				block_len
			}
			None => {
				let mut block_len = 8 + param_len + data_uncompressed_len;
				if self.checksum == Checksum::Crc32 {
					block_len += 4;
				}
				if self.inner.len() < block_len {
					return Ok(DeserialisedResult::MoreBytesRequired(
						block_len - self.inner.len(),
					));
				}
				block_len
			}
		};

		// Checksum check
		match self.checksum {
			Checksum::None => {}
			Checksum::Crc32 => {
				let bytes = try_from_slice::<4>(&self.inner[block_len - 4..block_len])?;
				let c = u32::from_le_bytes(bytes);
				let chk = crc32(&self.inner[..block_len - 4]);
				if c != chk {
					return Err(BinaryGcodeError::InvalidChecksum(c, chk));
				}
			}
		}

		let param_start = match data_compressed_len {
			Some(_) => 12,
			None => 8,
		};

		let encoding = &self.inner[param_start..param_start + 2];
		let encoding = try_from_slice::<2>(encoding)?;
		let encoding = Encoding::from_le_bytes(encoding, &kind)?;

		let parameters = self.inner[param_start..param_start + param_len]
			.to_owned()
			.into_boxed_slice();
		let data = self.inner[param_start + param_len..block_len - 4]
			.to_owned()
			.into_boxed_slice();

		// Pass out the block
		let b = DeserialisedBlock {
			kind,
			data_compressed_len,
			data_uncompressed_len,
			compression,
			encoding,
			parameters,
			data,
		};

		self.inner.drain(..block_len);

		Ok(DeserialisedResult::Block(b))
	}
}

#[derive(Debug)]
pub enum BlockError {
	DecodeError,
}

/// A struct representing a deserialised binary gcode block.
#[derive(Debug)]
pub struct DeserialisedBlock {
	pub kind: BlockKind,
	pub data_compressed_len: Option<usize>,
	pub data_uncompressed_len: usize,
	pub compression: CompressionAlgorithm,
	pub encoding: Encoding,
	pub parameters: Box<[u8]>,
	pub data: Box<[u8]>,
}

impl fmt::Display for DeserialisedBlock {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		write!(
			f,
			"{:?}  {{ compressed_len: {:?}, uncompressed_len: {}, compression: {:?}, encoding: {:?} }}",
			self.kind, self.data_compressed_len, self.data_uncompressed_len, self.compression, self.encoding
		)
	}
}

impl DeserialisedBlock {
	/// Internal function to decompress the data given the
	/// compression algorithm.
	pub fn decompress(&self) -> Result<Box<[u8]>, BlockError> {
		match self.compression {
			CompressionAlgorithm::None => Ok(self.data.clone()),
			CompressionAlgorithm::Deflate => {
				let output = decompress_to_vec_zlib(&self.data);
				if let Ok(o) = output {
					Ok(o.into_boxed_slice())
				} else {
					Err(BlockError::DecodeError)
				}
			}
			CompressionAlgorithm::Heatshrink11_4 => self.heatshrink(&self.data, 11, 4),
			CompressionAlgorithm::Heatshrink12_4 => self.heatshrink(&self.data, 12, 4),
		}
	}

	/// An internal function wrapping around the heatshrink decoder.
	fn heatshrink(
		&self,
		input: &[u8],
		window: u8,
		lookahead: u8,
	) -> Result<Box<[u8]>, BlockError> {
		let size = input.len() as u16;
		let mut decoder = HeatshrinkDecoder::new(size, window, lookahead).unwrap();
		decoder.sink(input);
		let mut data: Vec<u8> = vec![0; self.data_uncompressed_len];
		loop {
			let res = decoder.poll(&mut data);
			match res {
				HSDPollRes::Empty(_) => break,
				HSDPollRes::ErrorNull => return Err(BlockError::DecodeError),
				HSDPollRes::ErrorUnknown => return Err(BlockError::DecodeError),
				HSDPollRes::More(_) => {}
			}
		}
		Ok(data.into_boxed_slice())
	}
}
