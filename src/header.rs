#[derive(Debug, PartialEq)]
pub enum FileChecksum {
	None,
	Crc32,
}

impl FileChecksum {
	pub fn to_le_bytes(&self) -> [u8; 2] {
		match *self {
			FileChecksum::None => [0, 0],
			FileChecksum::Crc32 => [1, 0],
		}
	}

	pub fn checksum_byte_size(&self) -> usize {
		match *self {
			FileChecksum::None => 0,
			FileChecksum::Crc32 => 4,
		}
	}
}

#[derive(Debug, PartialEq)]
pub enum FileHeaderError {
	InvalidMagic,
	InvalidChecksum,
}

// ['G', 'C', 'D', 'E'] -> [u8; 4] -> u32
const MAGIC: u32 = 1162101575;

/// A struct containing the header information for a Binary GCode stream
/// (<https://github.com/prusa3d/libbgcode/blob/main/doc/specifications.md>).
///
/// |               | type     | size    | description                        |
/// | ------------- | -------- | ------- | ---------------------------------- |
/// | Magic Number  | uint32_t | 4 bytes | GCDE                               |
/// | Version       | uint32_t | 4 bytes | Version of the G-code binarization |
/// | Checksum type | uint16_t | 2 bytes | Algorithm used for checksum        |
///
#[derive(Debug)]
pub struct FileHeader {
	pub magic: u32,
	pub version: u32,
	pub checksum: FileChecksum,
}

impl FileHeader {
	pub fn new(
		version: u32,
		checksum: FileChecksum,
	) -> Self {
		Self {
			magic: MAGIC,
			version,
			checksum,
		}
	}

	pub fn from_bytes(bytes: &[u8; 10]) -> Result<Self, FileHeaderError> {
		let magic_bytes: [u8; 4] = bytes[0..=3].try_into().unwrap();
		let magic = u32::from_le_bytes(magic_bytes);
		if magic != MAGIC {
			return Err(FileHeaderError::InvalidMagic);
		}

		let version_bytes: [u8; 4] = bytes[4..=7].try_into().unwrap();
		let version = u32::from_le_bytes(version_bytes);

		let checksum_bytes: [u8; 2] = bytes[8..=9].try_into().unwrap();
		let checksum_value = u16::from_le_bytes(checksum_bytes);

		let mut checksum: FileChecksum = FileChecksum::None;
		match checksum_value {
			1 => checksum = FileChecksum::Crc32,
			0 => {}
			_ => return Err(FileHeaderError::InvalidChecksum),
		}

		Ok(Self {
			magic,
			version,
			checksum,
		})
	}

	pub fn to_bytes(&self) -> [u8; 10] {
		let mut bytes = [0u8; 10];
		let magic = self.magic.to_le_bytes();
		bytes[0..=3].clone_from_slice(&magic);
		let version = self.version.to_le_bytes();
		bytes[4..=7].clone_from_slice(&version);
		let checksum = self.checksum.to_le_bytes();
		bytes[8..=9].clone_from_slice(&checksum);
		bytes
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use super::FileHeader;

	#[test]
	fn test_valid_file_header_crc() {
		// Valid Magic, Version: 1, CRC encoding
		let bytes = [71, 67, 68, 69, 1, 0, 0, 0, 1, 0];
		let header = FileHeader::from_bytes(&bytes);
		assert!(header.is_ok());
		let header = header.unwrap();
		assert_eq!(header.checksum, FileChecksum::Crc32);
		assert_eq!(header.version, 1);
	}

	#[test]
	fn test_valid_file_header_no_crc() {
		// Valid Magic, Version: 1, CRC encoding
		let bytes = [71, 67, 68, 69, 1, 0, 0, 0, 0, 0];
		let header = FileHeader::from_bytes(&bytes);
		assert!(header.is_ok());
		let header = header.unwrap();
		assert_eq!(header.checksum, FileChecksum::None);
		assert_eq!(header.version, 1);
	}

	#[test]
	fn test_valid_file_header_version_2() {
		// Valid Magic, Version: 1, CRC encoding
		let bytes = [71, 67, 68, 69, 2, 0, 0, 0, 0, 0];
		let header = FileHeader::from_bytes(&bytes);
		assert!(header.is_ok());
		let header = header.unwrap();
		assert_eq!(header.checksum, FileChecksum::None);
		assert_eq!(header.version, 2);
	}

	#[test]
	fn test_invalid_magic() {
		let bytes = [72, 67, 68, 69, 1, 0, 0, 0, 1, 0];
		let header = FileHeader::from_bytes(&bytes);
		assert!(header.is_err());
		let header = header.err().unwrap();
		assert_eq!(header, FileHeaderError::InvalidMagic);
	}

	#[test]
	fn test_to_bytes() {
		let header = FileHeader::new(1, FileChecksum::Crc32);
		let bytes = header.to_bytes();
		assert_eq!(bytes, [71, 67, 68, 69, 1, 0, 0, 0, 1, 0])
	}
}
