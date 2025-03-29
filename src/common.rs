use core::array::TryFromSliceError;

// ['G', 'C', 'D', 'E'] -> [u8; 4] -> u32
pub(crate) const MAGIC: u32 = 1162101575;

#[derive(Debug, PartialEq, Eq)]
pub enum BinaryGcodeError {
	UnsupportedCompressionAlgorithm(u16),
	UnsupportedBlockKind(u16),
	IsNotCompressed,
	DataLengthMissMatch,
	TryFromSliceError,
	EncodingError(u16),
	DeflateError,
	HeatshrinkError,
	InvalidBlockConfig,
	InvalidMagic,
	InvalidChecksum,
}

/// An enum containing the various encodings the blocks
/// could contain.
pub enum Encoding {
	INI,
	ASCII,
	Meatpack,
	MeatpackWithComments,
	PNG,
	JPG,
	QOI,
}

impl Encoding {
	/// Returns the binary representation of the encoding.
	pub fn to_le_bytes(&self) -> [u8; 2] {
		match *self {
			Encoding::INI => 0u16.to_le_bytes(),
			Encoding::ASCII => 0u16.to_le_bytes(),
			Encoding::Meatpack => 1u16.to_le_bytes(),
			Encoding::MeatpackWithComments => 2u16.to_le_bytes(),
			Encoding::PNG => 0u16.to_le_bytes(),
			Encoding::JPG => 1u16.to_le_bytes(),
			Encoding::QOI => 2u16.to_le_bytes(),
		}
	}

	/// Returns the encoding type if or error if it is an invalid
	/// encoding combination.
	pub fn from_le_bytes(
		&self,
		bytes: [u8; 2],
		kind: BlockKind,
	) -> Result<Encoding, BinaryGcodeError> {
		let encoding = u16::from_le_bytes(bytes);
		match (kind, encoding) {
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
}

/// Defines the various kinds of block that are
/// in the binary gcode specification.
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
	/// Return a BlockKind based on a u16.
	pub fn new(value: u16) -> Result<Self, BinaryGcodeError> {
		match value {
			0 => Ok(Self::FileMetadata),
			1 => Ok(Self::GCode),
			2 => Ok(Self::SlicerMetadata),
			3 => Ok(Self::PrinterMetadata),
			4 => Ok(Self::PrintMetadata),
			5 => Ok(Self::Thumbnail),
			v => Err(BinaryGcodeError::UnsupportedBlockKind(v)),
		}
	}

	/// Returns the binary representation of the encoding.
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

	/// Returns a BlockKind or error from a byte representation.
	pub fn from_le_bytes(bytes: [u8; 2]) -> Result<Self, BinaryGcodeError> {
		let value = u16::from_le_bytes(bytes);
		BlockKind::new(value)
	}

	/// Return the expected parameter byte size length.
	pub fn parameter_byte_size(&self) -> usize {
		match *self {
			BlockKind::Thumbnail => 6,
			_ => 2,
		}
	}
}

/// Defines the varius compressions algorithms used in
/// binary gcode.
#[derive(Debug, PartialEq, Eq)]
pub enum CompressionAlgorithm {
	None,
	Deflate,        // ZLib encoded version.
	Heatshrink11_4, // Window + Lookahead
	Heatshrink12_4,
}

impl CompressionAlgorithm {
	/// Return a compression enum based on a u16.
	pub fn new(value: u16) -> Result<Self, BinaryGcodeError> {
		match value {
			0 => Ok(Self::None),
			1 => Ok(Self::Deflate),
			2 => Ok(Self::Heatshrink11_4),
			3 => Ok(Self::Heatshrink12_4),
			v => Err(BinaryGcodeError::UnsupportedCompressionAlgorithm(v)),
		}
	}

	/// Return the binary representation of the compression algorithm.
	pub fn to_le_bytes(&self) -> [u8; 2] {
		match *self {
			CompressionAlgorithm::None => 0u16.to_le_bytes(),
			CompressionAlgorithm::Deflate => 1u16.to_be_bytes(),
			CompressionAlgorithm::Heatshrink11_4 => 2u16.to_le_bytes(),
			CompressionAlgorithm::Heatshrink12_4 => 3u16.to_le_bytes(),
		}
	}

	/// Return the compression type or error based on a binary representation.
	pub fn from_le_bytes(bytes: [u8; 2]) -> Result<Self, BinaryGcodeError> {
		let value = u16::from_le_bytes(bytes);
		CompressionAlgorithm::new(value)
	}
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

#[derive(Debug, PartialEq)]
pub enum BinaryGcodeChecksum {
	None,
	Crc32,
}

impl BinaryGcodeChecksum {
	pub fn to_le_bytes(&self) -> [u8; 2] {
		match *self {
			BinaryGcodeChecksum::None => [0, 0],
			BinaryGcodeChecksum::Crc32 => [1, 0],
		}
	}

	pub fn checksum_byte_size(&self) -> usize {
		match *self {
			BinaryGcodeChecksum::None => 0,
			BinaryGcodeChecksum::Crc32 => 4,
		}
	}
}
