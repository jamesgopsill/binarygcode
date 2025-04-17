use core::{array::TryFromSliceError, fmt};

use alloc::{borrow::ToOwned, boxed::Box, vec::Vec};
use base64::{prelude::BASE64_STANDARD, Engine};
use embedded_heatshrink::{HSDFinishRes, HSDPollRes, HSDSinkRes, HeatshrinkDecoder};
use meatpack::Unpacker;
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
			Some(compressed_len) => {
				// header + parameters + comrpessed_len
				let mut block_len = 12 + param_len + compressed_len;
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
	DecodeError(&'static str),
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
					Err(BlockError::DecodeError("deflate"))
				}
			}
			CompressionAlgorithm::Heatshrink11_4 => {
				unshrink(&self.data, self.data_uncompressed_len, 11, 4)
			}
			CompressionAlgorithm::Heatshrink12_4 => {
				unshrink(&self.data, self.data_uncompressed_len, 12, 4)
			}
		}
	}

	/// Pumps the decompressed ascii representation
	/// of the gcode block into a buffer.
	pub fn to_ascii(
		&mut self,
		buf: &mut Vec<u8>,
	) -> Result<(), BinaryGcodeError> {
		let data = self.decompress().unwrap();
		match self.kind {
			BlockKind::FileMetadata => {
				buf.extend("; [FILE_METADATA_START]\n".as_bytes());
				// Handle first byte
				if data[0] != 59 {
					buf.extend("; ".as_bytes());
				}
				// Handle the window of bytes
				for win in data.windows(2) {
					// Check if the next line has already been commented
					// If not then add one.
					match win {
						[10, 59] => buf.push(win[0]),
						[10, _] => buf.extend("\n; ".as_bytes()),
						_ => buf.push(win[0]),
					}
				}
				// Make sure we add the last byte
				buf.push(*data.last().unwrap());
				// Add a new line.
				buf.extend("\n; [FILE_METADATA_END]\n".as_bytes());
			}
			BlockKind::PrinterMetadata => {
				buf.extend("; [PRINTER_METADATA_START]\n".as_bytes());
				// Handle first byte
				if data[0] != 59 {
					buf.extend("; ".as_bytes());
				}
				// Handle the window of bytes
				for win in data.windows(2) {
					// Check if the next line has already been commented
					// If not then add one.
					match win {
						[10, 59] => buf.push(win[0]),
						[10, _] => buf.extend("\n; ".as_bytes()),
						_ => buf.push(win[0]),
					}
				}
				// Make sure we add the last byte
				buf.push(*data.last().unwrap());
				buf.extend("\n; [PRINTER_METADATA_END]\n".as_bytes());
			}
			BlockKind::PrintMetadata => {
				buf.extend("; [PRINT_METADATA_START]\n".as_bytes());
				// Handle first byte
				if data[0] != 59 {
					buf.extend("; ".as_bytes());
				}
				// Handle the window of bytes
				for win in data.windows(2) {
					// Check if the next line has already been commented
					// If not then add one.
					match win {
						[10, 59] => buf.push(win[0]),
						[10, _] => buf.extend("\n; ".as_bytes()),
						_ => buf.push(win[0]),
					}
				}
				// Make sure we add the last byte
				buf.push(*data.last().unwrap());
				buf.extend("\n; [PRINT_METADATA_END]\n".as_bytes());
			}
			BlockKind::SlicerMetadata => {
				buf.extend("; [SLICER_METADATA_START]\n".as_bytes());
				// Handle first byte
				if data[0] != 59 {
					buf.extend("; ".as_bytes());
				}
				// Handle the window of bytes
				for win in data.windows(2) {
					// Check if the next line has already been commented
					// If not then add one.
					match win {
						[10, 59] => buf.push(win[0]),
						[10, _] => buf.extend("\n; ".as_bytes()),
						_ => buf.push(win[0]),
					}
				}
				// Make sure we add the last byte
				buf.push(*data.last().unwrap());
				buf.extend("\n; [SLICER_METADATA_END]\n".as_bytes());
			}
			BlockKind::Thumbnail => {
				//buf.resize_with(buf.len() + data.len(), Default::default);
				let width = try_from_slice::<2>(&self.parameters[2..=3])?;
				let width = u16::from_le_bytes(width);
				let height = try_from_slice::<2>(&self.parameters[4..=5])?;
				let height = u16::from_le_bytes(height);

				buf.extend("; [THUMBNAIL_START]\n".as_bytes());
				let r = BASE64_STANDARD.encode(&data).into_bytes();
				match self.encoding {
					Encoding::PNG => {
						let header =
							format!("; thumbnail begin {}x{} {}\n", width, height, r.len());
						buf.extend(header.as_bytes());
					}
					Encoding::JPG => {
						let header =
							format!("; thumbnail_JPG begin {}x{} {}\n", width, height, r.len());
						buf.extend(header.as_bytes());
					}
					Encoding::QOI => {
						let header =
							format!("; thumbnail_QOI begin {}x{} {}\n", width, height, r.len());
						buf.extend(header.as_bytes());
					}
					_ => {}
				}
				// Taking the max row length of 78 from libbgcode
				for chunk in r.chunks(78) {
					buf.extend("; ".as_bytes());
					buf.extend(chunk);
					buf.push(10);
				}
				match self.encoding {
					Encoding::PNG => buf.extend("; thumbnail end \n;\n".as_bytes()),
					Encoding::JPG => buf.extend("; thumbnail_JPG end \n;\n".as_bytes()),
					Encoding::QOI => buf.extend("; thumbnail_QOI end \n;\n".as_bytes()),
					_ => {}
				}
				buf.extend("; [THUMBNAIL_END]\n".as_bytes());
			}
			BlockKind::GCode => {
				buf.extend("; [GCODE_START]\n".as_bytes());
				match self.encoding {
					Encoding::ASCII => buf.extend(data),
					Encoding::Meatpack => {
						// Use the Meatpack crate to re-encode back to ASCII Gcode.
						if Unpacker::<64>::unpack_slice(&data, buf).is_err() {
							return Err(BinaryGcodeError::MeatpackError);
						}
					}
					Encoding::MeatpackWithComments => {
						if Unpacker::<64>::unpack_slice(&data, buf).is_err() {
							return Err(BinaryGcodeError::MeatpackError);
						}
					}
					_ => {}
				}
				buf.extend("; [GCODE_END]\n".as_bytes());
			}
		}
		Ok(())
	}
}

/// An internal function wrapping around the heatshrink decoder.
fn unshrink(
	input: &[u8],
	uncompressed_len: usize,
	window: u8,
	lookahead: u8,
) -> Result<Box<[u8]>, BlockError> {
	let input_buffer_size = input.len();
	let mut decoder = HeatshrinkDecoder::new(input_buffer_size as u16, window, lookahead).unwrap();
	let mut uncompressed: Vec<u8> = vec![0; uncompressed_len];
	let mut sunk: usize = 0;
	let mut polled: usize = 0;

	while sunk < input_buffer_size {
		match decoder.sink(&input[sunk..]) {
			HSDSinkRes::Ok(sz) => {
				sunk += sz;
			}
			HSDSinkRes::Full => return Err(BlockError::DecodeError("HSDSinkRes::Full")),
			HSDSinkRes::ErrorNull => return Err(BlockError::DecodeError("HSDSinkRes::ErrorNull")),
		}
		loop {
			let res = decoder.poll(&mut uncompressed[polled..]);
			match res {
				HSDPollRes::Empty(sz) => {
					polled += sz;
					if sz == 0 {
						break;
					}
				}
				// Panics after looping for more. Is there a bug where
				// More is Empty and Empty is More or my interpretation
				// of what they mean?
				HSDPollRes::More(sz) => {
					polled += sz;
					break;
				}
				HSDPollRes::ErrorNull => {
					return Err(BlockError::DecodeError("HSDPollRes::ErrorNull"))
				}
				HSDPollRes::ErrorUnknown => {
					return Err(BlockError::DecodeError("HSDPollRes::ErrorUnknown"))
				}
			}
		}
	}

	loop {
		match decoder.finish() {
			HSDFinishRes::Done => break,
			HSDFinishRes::More => match decoder.poll(&mut uncompressed[polled..]) {
				HSDPollRes::Empty(sz) => {
					polled += sz;
					if sz == 0 {
						break;
					}
				}
				HSDPollRes::More(sz) => {
					polled += sz;
				}
				HSDPollRes::ErrorUnknown => {
					return Err(BlockError::DecodeError("HSDPollRes::ErrorUnknown"))
				}
				HSDPollRes::ErrorNull => {
					return Err(BlockError::DecodeError("HSDPollRes::ErrorNull"))
				}
			},
			HSDFinishRes::ErrorNull => {
				return Err(BlockError::DecodeError("HSDFinishRes::ErrorNull"))
			}
		}
	}

	Ok(uncompressed.into_boxed_slice())
}
