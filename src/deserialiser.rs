use core::array::TryFromSliceError;

use alloc::vec::Vec;

use crate::header::FileChecksum;

#[derive(Debug)]
pub enum BlockDeserialiserError {
	UnsupportedCompressionAlgorithm(u16),
	UnsupportedBlockKind(u16),
	IsNotCompressed,
	DataLengthMissMatch,
	TryFromSliceError,
	EncodingError(u16),
}

#[derive(Debug)]
pub enum BlockKind {
	FileMetadata,
	GCode,
	SlicerMetadata,
	PrinterMetadata,
	PrintMetadata,
	Thumbnail,
}

impl BlockKind {
	pub fn new(value: u16) -> Result<Self, BlockDeserialiserError> {
		match value {
			0 => Ok(Self::FileMetadata),
			1 => Ok(Self::GCode),
			2 => Ok(Self::SlicerMetadata),
			3 => Ok(Self::PrinterMetadata),
			4 => Ok(Self::PrintMetadata),
			5 => Ok(Self::Thumbnail),
			v => Err(BlockDeserialiserError::UnsupportedBlockKind(v)),
		}
	}

	pub fn to_le_bytes(&self) -> [u8; 2] {
		match *self {
			BlockKind::FileMetadata => 0u16.to_le_bytes(),
			BlockKind::GCode => 1u16.to_be_bytes(),
			BlockKind::SlicerMetadata => 1u16.to_le_bytes(),
			BlockKind::PrinterMetadata => 2u16.to_le_bytes(),
			BlockKind::PrintMetadata => 3u16.to_le_bytes(),
			BlockKind::Thumbnail => 4u16.to_le_bytes(),
		}
	}

	pub fn from_le_bytes(bytes: [u8; 2]) -> Result<Self, BlockDeserialiserError> {
		let value = u16::from_le_bytes(bytes);
		BlockKind::new(value)
	}

	pub fn parameter_byte_size(&self) -> usize {
		match *self {
			BlockKind::Thumbnail => 6,
			_ => 2,
		}
	}
}

#[derive(Debug, PartialEq, Eq)]
pub enum CompressionAlgorithm {
	None,
	Deflate, // ZLib encoded version.
	// Could one day move to a move general Heatshrink(u8, u8) if they all
	// featured a respective and reserved [u8; 2] id.
	Heatshrink11_4, // Window + Lookahead
	Heatshrink12_4,
}

impl CompressionAlgorithm {
	pub fn new(value: u16) -> Result<Self, BlockDeserialiserError> {
		match value {
			0 => Ok(Self::None),
			1 => Ok(Self::Deflate),
			2 => Ok(Self::Heatshrink11_4),
			3 => Ok(Self::Heatshrink12_4),
			v => Err(BlockDeserialiserError::UnsupportedCompressionAlgorithm(v)),
		}
	}

	pub fn to_le_bytes(&self) -> [u8; 2] {
		match *self {
			CompressionAlgorithm::None => 0u16.to_le_bytes(),
			CompressionAlgorithm::Deflate => 1u16.to_be_bytes(),
			CompressionAlgorithm::Heatshrink11_4 => 2u16.to_le_bytes(),
			CompressionAlgorithm::Heatshrink12_4 => 3u16.to_le_bytes(),
		}
	}

	pub fn from_le_bytes(bytes: [u8; 2]) -> Result<Self, BlockDeserialiserError> {
		let value = u16::from_le_bytes(bytes);
		CompressionAlgorithm::new(value)
	}
}

fn try_from_slice<const N: usize>(buf: &[u8]) -> Result<[u8; N], BlockDeserialiserError> {
	let bytes: Result<[u8; N], TryFromSliceError> = buf.try_into();
	match bytes {
		Ok(bytes) => Ok(bytes),
		Err(_) => Err(BlockDeserialiserError::TryFromSliceError),
	}
}

#[derive(Debug)]
pub struct BlockDeserialiser {
	buf: Vec<u8>,
	checksum: FileChecksum,
}

impl BlockDeserialiser {
	pub fn new(checksum: FileChecksum) -> Self {
		Self {
			buf: Vec::with_capacity(12),
			checksum,
		}
	}

	pub fn kind(&self) -> Result<BlockKind, BlockDeserialiserError> {
		let bytes = try_from_slice::<2>(&self.buf[0..=1])?;
		BlockKind::from_le_bytes(bytes)
	}

	pub fn compression(&self) -> Result<CompressionAlgorithm, BlockDeserialiserError> {
		let bytes = try_from_slice::<2>(&self.buf[2..=3])?;
		CompressionAlgorithm::from_le_bytes(bytes)
	}

	pub fn compressed_size(&self) -> Result<usize, BlockDeserialiserError> {
		let ca = self.compression()?;
		match ca {
			CompressionAlgorithm::None => Err(BlockDeserialiserError::IsNotCompressed),
			_ => {
				let bytes = try_from_slice::<4>(&self.buf[8..=11])?;
				Ok(u32::from_le_bytes(bytes) as usize)
			}
		}
	}

	pub fn uncompressed_size(&self) -> Result<usize, BlockDeserialiserError> {
		let bytes = try_from_slice::<4>(&self.buf[4..=7])?;
		Ok(u32::from_le_bytes(bytes) as usize)
	}

	pub fn block_size(&self) -> Result<usize, BlockDeserialiserError> {
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

	pub fn header_buf(&mut self) -> &mut [u8] {
		self.buf = Vec::with_capacity(12);
		for _ in 0..self.buf.capacity() {
			self.buf.push(0);
		}
		self.buf.as_mut()
	}

	pub fn data_buf(&mut self) -> Result<&mut [u8], BlockDeserialiserError> {
		let additional = self.block_size()?;
		self.buf.reserve_exact(additional);
		for _ in 0..additional {
			self.buf.push(0);
		}
		let slice = self.buf[12..].as_mut();
		Ok(slice)
	}

	pub fn deserialise(&self) -> Result<Vec<u8>, BlockDeserialiserError> {
		// Check the expected and received lengths
		// The user may have forgetton to read in the data
		let buf_length_check = 12 + self.block_size()?;
		if buf_length_check != self.buf.len() {
			return Err(BlockDeserialiserError::DataLengthMissMatch);
		}

		match self.kind()? {
			BlockKind::FileMetadata => self.deserialise_ini_data(),
			BlockKind::GCode => todo!(),
			BlockKind::PrintMetadata => self.deserialise_ini_data(),
			BlockKind::PrinterMetadata => self.deserialise_ini_data(),
			BlockKind::SlicerMetadata => self.deserialise_ini_data(),
			BlockKind::Thumbnail => self.deserialise_thumbnail_data(),
		}
	}

	fn deserialise_thumbnail_data(&self) -> Result<Vec<u8>, BlockDeserialiserError> {
		let data: Vec<u8> = Vec::new();
		let c = self.compression()?;
		let mut idx: usize;
		match c {
			CompressionAlgorithm::None => idx = 8,
			_ => idx = 12,
		}
		let encoding = try_from_slice::<2>(&self.buf[idx..=idx + 1])?;
		let encoding = u16::from_le_bytes(encoding);
		if encoding > 2 {
			return Err(BlockDeserialiserError::EncodingError(encoding));
		}
		// Start of the data
		let start = idx + 2;
		let mut end: usize;
		match self.checksum {
			FileChecksum::None => end = self.buf.len(),
			FileChecksum::Crc32 => {
				end = self.buf.len() - 4;
				let checksum = &self.buf[end..];
				// TODO: deal with the checksum
			}
		}

		// Deal with the data
		let data = self.deserialise_data(start, end)?;

		// Then the encoding (if required)
		Ok(data)
	}

	fn deserialise_ini_data(&self) -> Result<Vec<u8>, BlockDeserialiserError> {
		let data: Vec<u8> = Vec::new();
		let c = self.compression()?;
		let mut idx: usize;
		match c {
			CompressionAlgorithm::None => idx = 8,
			_ => idx = 12,
		}
		let encoding = try_from_slice::<2>(&self.buf[idx..=idx + 1])?;
		let encoding = u16::from_le_bytes(encoding);
		if encoding != 0 {
			return Err(BlockDeserialiserError::EncodingError(encoding));
		}
		// Start of the data
		let start = idx + 2;
		let mut end: usize;
		match self.checksum {
			FileChecksum::None => end = self.buf.len(),
			FileChecksum::Crc32 => {
				end = self.buf.len() - 4;
				let checksum = &self.buf[end..];
				// TODO: deal with the checksum
			}
		}

		// Deal with the data
		let data = self.deserialise_data(start, end)?;

		// Then the encoding (if required)
		Ok(data)
	}

	fn deserialise_data(
		&self,
		start: usize,
		end: usize,
	) -> Result<Vec<u8>, BlockDeserialiserError> {
		let mut data: Vec<u8> = Vec::new();
		match self.compression()? {
			CompressionAlgorithm::None => {
				for v in self.buf[start..end].iter() {
					data.push(*v);
				}
			}
			CompressionAlgorithm::Deflate => {
				todo!()
			}
			CompressionAlgorithm::Heatshrink11_4 => {
				todo!()
			}
			CompressionAlgorithm::Heatshrink12_4 => {
				todo!()
			}
		}
		Ok(data)
	}
}
