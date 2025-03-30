use alloc::vec::Vec;
use embedded_heatshrink::{HSDPollRes, HeatshrinkDecoder};
use miniz_oxide::inflate::decompress_to_vec_zlib;

use crate::common::{
	crc32, try_from_slice, BinaryGcodeChecksum, BinaryGcodeError, BlockKind, CompressionAlgorithm,
	Encoding, MAGIC,
};

/// A binarygcode deserialiser that can parse a bgcode file. It can
/// separately read block headers and content so you can choose what
/// data you want to parse when reading the file.
#[derive(Debug)]
pub struct Deserialiser {
	pub magic: u32,
	pub version: u32,
	pub checksum: BinaryGcodeChecksum,
	buf: Vec<u8>,
}

impl Deserialiser {
	/// Create a new deserialiser by first passing a buffer containing the file
	/// header information
	/// (<https://github.com/prusa3d/libbgcode/blob/main/doc/specifications.md>).
	///
	/// |               | type     | size    | description                        |
	/// | ------------- | -------- | ------- | ---------------------------------- |
	/// | Magic Number  | uint32_t | 4 bytes | GCDE                               |
	/// | Version       | uint32_t | 4 bytes | Version of the G-code binarization |
	/// | Checksum type | uint16_t | 2 bytes | Algorithm used for checksum        |
	///
	/// Returns a valid deserialiser if the header data is read successfully or
	/// an error enum detailing the header issue.
	pub fn new(header_bytes: &[u8; 10]) -> Result<Self, BinaryGcodeError> {
		let bytes = try_from_slice::<4>(&header_bytes[0..=3])?;
		let magic = u32::from_le_bytes(bytes);
		if magic != MAGIC {
			return Err(BinaryGcodeError::InvalidMagic);
		}

		let bytes = try_from_slice::<4>(&header_bytes[4..=7])?;
		let version = u32::from_le_bytes(bytes);

		let bytes = try_from_slice::<2>(&header_bytes[8..=9])?;
		let checksum_value = u16::from_le_bytes(bytes);

		let checksum = match checksum_value {
			1 => BinaryGcodeChecksum::Crc32,
			0 => BinaryGcodeChecksum::None,
			_ => return Err(BinaryGcodeError::InvalidChecksumType),
		};

		let s = Self {
			magic,
			version,
			checksum,
			buf: Vec::with_capacity(12),
		};

		Ok(s)
	}

	/// A utility function that creates a buffer of the size of the file header
	/// so it can be read into and passed to a new instance of Deserialiser.
	pub fn fh_buf() -> [u8; 10] {
		[0u8; 10]
	}

	/// Returns the type of block that is currently in the deserialiser buffer.
	pub fn kind(&self) -> Result<BlockKind, BinaryGcodeError> {
		let bytes = try_from_slice::<2>(&self.buf[0..=1])?;
		BlockKind::from_le_bytes(bytes)
	}

	/// Returns the compression alogrithm being used by the current block in the
	/// deserialiser buffer.
	pub fn compression(&self) -> Result<CompressionAlgorithm, BinaryGcodeError> {
		let bytes = try_from_slice::<2>(&self.buf[2..=3])?;
		CompressionAlgorithm::from_le_bytes(bytes)
	}

	/// Returns the encoding used by the current block in the deserialiser buffer.
	pub fn encoding(&self) -> Result<Encoding, BinaryGcodeError> {
		let start = match self.compression()? {
			CompressionAlgorithm::None => 8,
			_ => 12,
		};
		let end = start + 2;

		let encoding = &self.buf[start..end];
		let encoding = try_from_slice::<2>(encoding)?;
		let encoding = u16::from_le_bytes(encoding);

		// Check te encoding is valid
		match (self.kind()?, encoding) {
			(BlockKind::FileMetadata, 0) => Ok(Encoding::INI),
			(BlockKind::SlicerMetadata, 0) => Ok(Encoding::INI),
			(BlockKind::PrintMetadata, 0) => Ok(Encoding::INI),
			(BlockKind::PrinterMetadata, 0) => Ok(Encoding::INI),
			(BlockKind::GCode, 0) => Ok(Encoding::ASCII),
			(BlockKind::GCode, 1) => Ok(Encoding::Meatpack),
			(BlockKind::GCode, 2) => Ok(Encoding::MeatpackWithComments),
			(BlockKind::Thumbnail, 0) => Ok(Encoding::PNG),
			(BlockKind::Thumbnail, 1) => Ok(Encoding::JPG),
			(BlockKind::Thumbnail, 2) => Ok(Encoding::QOI),
			(_, _) => Err(BinaryGcodeError::EncodingError(encoding)),
		}
	}

	/// Returns the compressed size of the block in the deserialiser buffer.
	pub fn compressed_size(&self) -> Result<usize, BinaryGcodeError> {
		let ca = self.compression()?;
		match ca {
			CompressionAlgorithm::None => Err(BinaryGcodeError::IsNotCompressed),
			_ => {
				let bytes = try_from_slice::<4>(&self.buf[8..=11])?;
				Ok(u32::from_le_bytes(bytes) as usize)
			}
		}
	}

	/// Returns the uncompressed size of the block in the deserialiser buffer.
	pub fn uncompressed_size(&self) -> Result<usize, BinaryGcodeError> {
		let bytes = try_from_slice::<4>(&self.buf[4..=7])?;
		Ok(u32::from_le_bytes(bytes) as usize)
	}

	/// Returns the size of the block (minus header) that can be used to skip
	/// forward in reading the file if you do not want to process it.
	pub fn block_size(&self) -> Result<usize, BinaryGcodeError> {
		let mut size: usize = 0;
		size += self.kind()?.parameter_byte_size();
		size += self.checksum.checksum_byte_size();
		let c = self.compression()?;
		match c {
			CompressionAlgorithm::None => {
				size -= 4; // less four bytes as we have already have and the header is actually [u8; 8].
				size += self.uncompressed_size()?;
			}
			_ => size += self.compressed_size()?,
		}
		Ok(size)
	}

	/// A utility function resetting the internal buffer for a new block that
	/// starts with a buffer size suitable for a new block header to be read into.
	pub fn block_header_buf(&mut self) -> &mut [u8] {
		self.buf = Vec::with_capacity(12);
		for _ in 0..self.buf.capacity() {
			self.buf.push(0);
		}
		self.buf.as_mut()
	}

	/// A utility function that reserve the exact memory required to read in the
	/// block data given the header information provided previously.
	pub fn block_data_buf(&mut self) -> Result<&mut [u8], BinaryGcodeError> {
		let additional = self.block_size()?;
		self.buf.reserve_exact(additional);
		for _ in 0..additional {
			self.buf.push(0);
		}
		let slice = self.buf[12..].as_mut();
		Ok(slice)
	}

	/// Deserialise a blocks data and return it as `Vec<u8>`.
	pub fn deserialise(&self) -> Result<Vec<u8>, BinaryGcodeError> {
		// Check the expected and received lengths
		// The user may have forgotten to read in the data
		let buf_length_check = 12 + self.block_size()?;
		if buf_length_check != self.buf.len() {
			return Err(BinaryGcodeError::DataLengthMissMatch);
		}

		let ss = self.slice_set()?;
		if let Some(c) = ss.checksum {
			let c = try_from_slice::<4>(&c[..4]).unwrap();
			let c = u32::from_le_bytes(c);
			let chk = crc32(ss.block);
			if c != chk {
				return Err(BinaryGcodeError::InvalidChecksum(c, chk));
			}
		}

		// Decompress the data
		let data = self.decompress(ss.data)?;
		Ok(data)
	}

	/// A utility function that generates the slices for the entire block, parameter,
	/// data, and checksum sections.
	fn slice_set(&self) -> Result<SliceSet, BinaryGcodeError> {
		let parameter_start = match self.compression()? {
			CompressionAlgorithm::None => 8,
			_ => 12,
		};
		let mut data_start = parameter_start;
		match self.kind()? {
			BlockKind::Thumbnail => data_start += 6,
			_ => data_start += 2,
		}

		// Now for the data and checksum slices
		let block = &self.buf[..self.buf.len() - 4];
		let data: &[u8];
		let checksum: Option<&[u8]>;

		match self.checksum {
			BinaryGcodeChecksum::None => {
				data = &self.buf[data_start..];
				checksum = None;
			}
			BinaryGcodeChecksum::Crc32 => {
				let end = self.buf.len() - 4;
				data = &self.buf[data_start..end];
				checksum = Some(&self.buf[end..]);
			}
		}

		Ok(SliceSet {
			data,
			block,
			checksum,
		})
	}

	/// Internal function to decompress the data given the
	/// compression algorithm.
	fn decompress(
		&self,
		input: &[u8],
	) -> Result<Vec<u8>, BinaryGcodeError> {
		match self.compression()? {
			CompressionAlgorithm::None => {
				let mut output: Vec<u8> = Vec::new();
				for v in input.iter() {
					output.push(*v);
				}
				Ok(output)
			}
			CompressionAlgorithm::Deflate => {
				let output = decompress_to_vec_zlib(input);
				if let Ok(o) = output {
					Ok(o)
				} else {
					Err(BinaryGcodeError::DeflateError)
				}
			}
			CompressionAlgorithm::Heatshrink11_4 => self.heatshrink(input, 11, 4),
			CompressionAlgorithm::Heatshrink12_4 => self.heatshrink(input, 12, 4),
		}
	}

	/// An internal function wrapping around the heatshrink decoder.
	fn heatshrink(
		&self,
		input: &[u8],
		window: u8,
		lookahead: u8,
	) -> Result<Vec<u8>, BinaryGcodeError> {
		let size = input.len() as u16;
		let mut decoder = HeatshrinkDecoder::new(size, window, lookahead).unwrap();
		decoder.sink(input);
		let mut data: Vec<u8> = vec![0; self.uncompressed_size()?];
		loop {
			let res = decoder.poll(&mut data);
			match res {
				HSDPollRes::Empty(_) => break,
				HSDPollRes::ErrorNull => return Err(BinaryGcodeError::HeatshrinkError),
				HSDPollRes::ErrorUnknown => return Err(BinaryGcodeError::HeatshrinkError),
				HSDPollRes::More(_) => {}
			}
		}
		Ok(data)
	}
}

/// An internal utility struct containing the various slices
/// of the buffer that represent different sections of a block.
struct SliceSet<'a> {
	data: &'a [u8],
	block: &'a [u8],
	checksum: Option<&'a [u8]>,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_valid_file_header_crc() {
		// Valid Magic, Version: 1, CRC encoding
		let bytes = [71, 67, 68, 69, 1, 0, 0, 0, 1, 0];
		let d = Deserialiser::new(&bytes);
		assert!(d.is_ok());
		let d = d.unwrap();
		assert_eq!(d.checksum, BinaryGcodeChecksum::Crc32);
		assert_eq!(d.version, 1);
	}

	#[test]
	fn test_valid_file_header_no_crc() {
		// Valid Magic, Version: 1, CRC encoding
		let bytes = [71, 67, 68, 69, 1, 0, 0, 0, 0, 0];
		let d = Deserialiser::new(&bytes);
		assert!(d.is_ok());
		let d = d.unwrap();
		assert_eq!(d.checksum, BinaryGcodeChecksum::None);
		assert_eq!(d.version, 1);
	}

	#[test]
	fn test_valid_file_header_version_2() {
		// Valid Magic, Version: 1, CRC encoding
		let bytes = [71, 67, 68, 69, 2, 0, 0, 0, 0, 0];
		let d = Deserialiser::new(&bytes);
		assert!(d.is_ok());
		let d = d.unwrap();
		assert_eq!(d.checksum, BinaryGcodeChecksum::None);
		assert_eq!(d.version, 2);
	}

	#[test]
	fn test_invalid_magic() {
		let bytes = [72, 67, 68, 69, 1, 0, 0, 0, 1, 0];
		let d = Deserialiser::new(&bytes);
		assert!(d.is_err());
		let d = d.err().unwrap();
		assert_eq!(d, BinaryGcodeError::InvalidMagic);
	}
}
