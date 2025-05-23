use alloc::string::String;
use meatpack::MeatPackError;
use thiserror::Error;

/// A pre-defined value in the specification as
/// part of the file header defined in its u32 form.
/// ['G', 'C', 'D', 'E'] -> [u8; 4] -> u32
pub(crate) static MAGIC: u32 = 1162101575;

/// An enum of errors that can occur when using the crate.
#[derive(Debug, Error)]
pub enum BinaryGcodeError {
    #[error("Error converting from slice.")]
    TryFromSliceError,
    #[error("Invalid MAGIC received. Expected 1162101575. Received {0}")]
    InvalidMagic(u32),
    #[error("Invalid checksum type. Expected 0-1. Received {0}")]
    InvalidChecksumType(u16),
    #[error("Invalid checksum received. Expected {0}. Received {1}")]
    InvalidChecksum(u32, u32),
    #[error("Unsupported blocksum kind. Expected 0-5. Received {0}")]
    UnsupportedBlockKind(u16),
    #[error("Unsupported encoding. Expected 0-2. Received {0}")]
    UnsupportedEncoding(u16),
    #[error("Unsupported compression algorithm. Received {0}")]
    UnsupportedCompressionAlgorithm(u16),
    #[error("Encoding Error. Received {0}")]
    EncodingError(u16),
    #[error("Meatpack Error")]
    Meatpack(#[from] MeatPackError),
    #[error("Serialise Error")]
    SerialiseError(&'static str),
    // A utility error during development
    // until to parse out the string as
    // in no_std mode.
    #[error("Development Error: {0}")]
    DevError(String),
}

/// The valid checksums for the binary gcode format.
#[derive(Debug, PartialEq, Clone)]
pub enum Checksum {
    None,
    Crc32,
}

impl Checksum {
    pub const fn to_le_bytes(&self) -> [u8; 2] {
        match *self {
            Checksum::None => 0u16.to_be_bytes(),
            Checksum::Crc32 => 1u16.to_le_bytes(),
        }
    }

    pub const fn checksum_byte_size(&self) -> usize {
        match *self {
            Checksum::None => 0,
            Checksum::Crc32 => 4,
        }
    }
}

/// An enum containing the various encodings the blocks
/// could contain.
#[derive(Debug)]
pub enum Encoding {
    Ini,
    Ascii,
    Meatpack,
    MeatpackWithComments,
    Png,
    Jpg,
    Qoi,
}

impl Encoding {
    /// Returns the binary representation of the encoding.
    pub const fn to_le_bytes(&self) -> [u8; 2] {
        match *self {
            Encoding::Ini => 0u16.to_le_bytes(),
            Encoding::Ascii => 0u16.to_le_bytes(),
            Encoding::Meatpack => 1u16.to_le_bytes(),
            Encoding::MeatpackWithComments => 2u16.to_le_bytes(),
            Encoding::Png => 0u16.to_le_bytes(),
            Encoding::Jpg => 1u16.to_le_bytes(),
            Encoding::Qoi => 2u16.to_le_bytes(),
        }
    }

    /// Returns the encoding type if or error if it is an invalid
    /// encoding combination.
    pub const fn from_le_bytes(
        bytes: [u8; 2],
        kind: &BlockKind,
    ) -> Result<Encoding, BinaryGcodeError> {
        let encoding = u16::from_le_bytes(bytes);
        match (kind, encoding) {
            (BlockKind::FileMetadata, 0) => Ok(Encoding::Ini),
            (BlockKind::SlicerMetadata, 0) => Ok(Encoding::Ini),
            (BlockKind::PrintMetadata, 0) => Ok(Encoding::Ini),
            (BlockKind::PrinterMetadata, 0) => Ok(Encoding::Ini),
            (BlockKind::GCode, 0) => Ok(Encoding::Ascii),
            (BlockKind::GCode, 1) => Ok(Encoding::Meatpack),
            (BlockKind::GCode, 2) => Ok(Encoding::MeatpackWithComments),
            (BlockKind::Thumbnail, 0) => Ok(Encoding::Png),
            (BlockKind::Thumbnail, 1) => Ok(Encoding::Jpg),
            (BlockKind::Thumbnail, 2) => Ok(Encoding::Qoi),
            (_, _) => Err(BinaryGcodeError::UnsupportedEncoding(encoding)),
        }
    }
}

/// Defines the various kinds of block that are
/// in the binary gcode specification.
#[derive(Debug, PartialEq, Eq)]
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
    pub const fn new(value: u16) -> Result<Self, BinaryGcodeError> {
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
    pub const fn to_le_bytes(&self) -> [u8; 2] {
        match *self {
            BlockKind::FileMetadata => 0u16.to_le_bytes(),
            BlockKind::GCode => 1u16.to_le_bytes(),
            BlockKind::SlicerMetadata => 2u16.to_le_bytes(),
            BlockKind::PrinterMetadata => 3u16.to_le_bytes(),
            BlockKind::PrintMetadata => 4u16.to_le_bytes(),
            BlockKind::Thumbnail => 5u16.to_le_bytes(),
        }
    }

    /// Returns a BlockKind or error from a byte representation.
    pub const fn from_le_bytes(
        bytes: [u8; 2]
    ) -> Result<Self, BinaryGcodeError> {
        let value = u16::from_le_bytes(bytes);
        BlockKind::new(value)
    }

    /// Return the expected parameter byte size length.
    pub const fn parameter_byte_size(&self) -> usize {
        match *self {
            BlockKind::Thumbnail => 6,
            _ => 2,
        }
    }
}

/// Defines the various compressions algorithms used in binary gcode.
#[derive(Debug, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    None,
    Deflate,        // ZLib encoded version.
    Heatshrink11_4, // Window + Lookahead
    Heatshrink12_4,
}

impl CompressionAlgorithm {
    /// Return a compression enum based on a u16.
    pub const fn new(value: u16) -> Result<Self, BinaryGcodeError> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Deflate),
            2 => Ok(Self::Heatshrink11_4),
            3 => Ok(Self::Heatshrink12_4),
            v => Err(BinaryGcodeError::UnsupportedCompressionAlgorithm(v)),
        }
    }

    /// Return the binary representation of the compression algorithm.
    pub const fn to_le_bytes(&self) -> [u8; 2] {
        match *self {
            CompressionAlgorithm::None => 0u16.to_le_bytes(),
            CompressionAlgorithm::Deflate => 1u16.to_le_bytes(),
            CompressionAlgorithm::Heatshrink11_4 => 2u16.to_le_bytes(),
            CompressionAlgorithm::Heatshrink12_4 => 3u16.to_le_bytes(),
        }
    }

    /// Return the compression type or error based on a binary representation.
    pub const fn from_le_bytes(
        bytes: [u8; 2]
    ) -> Result<Self, BinaryGcodeError> {
        let value = u16::from_le_bytes(bytes);
        CompressionAlgorithm::new(value)
    }
}

/// The crcfast implementation using a lookup table following
/// [lxp32](https://lxp32.github.io/docs/a-simple-example-crc32-calculation/)s example.
pub(crate) fn crc32(buf: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for byte in buf {
        let c = crc as u8;
        let idx = (c ^ byte) as usize;
        crc = CRC32_TABLE[idx] ^ (crc >> 8);
    }
    !crc
}

static CRC32_TABLE: [u32; 256] = [
    0x00000000, 0x77073096, 0xee0e612c, 0x990951ba, 0x076dc419, 0x706af48f,
    0xe963a535, 0x9e6495a3, 0x0edb8832, 0x79dcb8a4, 0xe0d5e91e, 0x97d2d988,
    0x09b64c2b, 0x7eb17cbd, 0xe7b82d07, 0x90bf1d91, 0x1db71064, 0x6ab020f2,
    0xf3b97148, 0x84be41de, 0x1adad47d, 0x6ddde4eb, 0xf4d4b551, 0x83d385c7,
    0x136c9856, 0x646ba8c0, 0xfd62f97a, 0x8a65c9ec, 0x14015c4f, 0x63066cd9,
    0xfa0f3d63, 0x8d080df5, 0x3b6e20c8, 0x4c69105e, 0xd56041e4, 0xa2677172,
    0x3c03e4d1, 0x4b04d447, 0xd20d85fd, 0xa50ab56b, 0x35b5a8fa, 0x42b2986c,
    0xdbbbc9d6, 0xacbcf940, 0x32d86ce3, 0x45df5c75, 0xdcd60dcf, 0xabd13d59,
    0x26d930ac, 0x51de003a, 0xc8d75180, 0xbfd06116, 0x21b4f4b5, 0x56b3c423,
    0xcfba9599, 0xb8bda50f, 0x2802b89e, 0x5f058808, 0xc60cd9b2, 0xb10be924,
    0x2f6f7c87, 0x58684c11, 0xc1611dab, 0xb6662d3d, 0x76dc4190, 0x01db7106,
    0x98d220bc, 0xefd5102a, 0x71b18589, 0x06b6b51f, 0x9fbfe4a5, 0xe8b8d433,
    0x7807c9a2, 0x0f00f934, 0x9609a88e, 0xe10e9818, 0x7f6a0dbb, 0x086d3d2d,
    0x91646c97, 0xe6635c01, 0x6b6b51f4, 0x1c6c6162, 0x856530d8, 0xf262004e,
    0x6c0695ed, 0x1b01a57b, 0x8208f4c1, 0xf50fc457, 0x65b0d9c6, 0x12b7e950,
    0x8bbeb8ea, 0xfcb9887c, 0x62dd1ddf, 0x15da2d49, 0x8cd37cf3, 0xfbd44c65,
    0x4db26158, 0x3ab551ce, 0xa3bc0074, 0xd4bb30e2, 0x4adfa541, 0x3dd895d7,
    0xa4d1c46d, 0xd3d6f4fb, 0x4369e96a, 0x346ed9fc, 0xad678846, 0xda60b8d0,
    0x44042d73, 0x33031de5, 0xaa0a4c5f, 0xdd0d7cc9, 0x5005713c, 0x270241aa,
    0xbe0b1010, 0xc90c2086, 0x5768b525, 0x206f85b3, 0xb966d409, 0xce61e49f,
    0x5edef90e, 0x29d9c998, 0xb0d09822, 0xc7d7a8b4, 0x59b33d17, 0x2eb40d81,
    0xb7bd5c3b, 0xc0ba6cad, 0xedb88320, 0x9abfb3b6, 0x03b6e20c, 0x74b1d29a,
    0xead54739, 0x9dd277af, 0x04db2615, 0x73dc1683, 0xe3630b12, 0x94643b84,
    0x0d6d6a3e, 0x7a6a5aa8, 0xe40ecf0b, 0x9309ff9d, 0x0a00ae27, 0x7d079eb1,
    0xf00f9344, 0x8708a3d2, 0x1e01f268, 0x6906c2fe, 0xf762575d, 0x806567cb,
    0x196c3671, 0x6e6b06e7, 0xfed41b76, 0x89d32be0, 0x10da7a5a, 0x67dd4acc,
    0xf9b9df6f, 0x8ebeeff9, 0x17b7be43, 0x60b08ed5, 0xd6d6a3e8, 0xa1d1937e,
    0x38d8c2c4, 0x4fdff252, 0xd1bb67f1, 0xa6bc5767, 0x3fb506dd, 0x48b2364b,
    0xd80d2bda, 0xaf0a1b4c, 0x36034af6, 0x41047a60, 0xdf60efc3, 0xa867df55,
    0x316e8eef, 0x4669be79, 0xcb61b38c, 0xbc66831a, 0x256fd2a0, 0x5268e236,
    0xcc0c7795, 0xbb0b4703, 0x220216b9, 0x5505262f, 0xc5ba3bbe, 0xb2bd0b28,
    0x2bb45a92, 0x5cb36a04, 0xc2d7ffa7, 0xb5d0cf31, 0x2cd99e8b, 0x5bdeae1d,
    0x9b64c2b0, 0xec63f226, 0x756aa39c, 0x026d930a, 0x9c0906a9, 0xeb0e363f,
    0x72076785, 0x05005713, 0x95bf4a82, 0xe2b87a14, 0x7bb12bae, 0x0cb61b38,
    0x92d28e9b, 0xe5d5be0d, 0x7cdcefb7, 0x0bdbdf21, 0x86d3d2d4, 0xf1d4e242,
    0x68ddb3f8, 0x1fda836e, 0x81be16cd, 0xf6b9265b, 0x6fb077e1, 0x18b74777,
    0x88085ae6, 0xff0f6a70, 0x66063bca, 0x11010b5c, 0x8f659eff, 0xf862ae69,
    0x616bffd3, 0x166ccf45, 0xa00ae278, 0xd70dd2ee, 0x4e048354, 0x3903b3c2,
    0xa7672661, 0xd06016f7, 0x4969474d, 0x3e6e77db, 0xaed16a4a, 0xd9d65adc,
    0x40df0b66, 0x37d83bf0, 0xa9bcae53, 0xdebb9ec5, 0x47b2cf7f, 0x30b5ffe9,
    0xbdbdf21c, 0xcabac28a, 0x53b39330, 0x24b4a3a6, 0xbad03605, 0xcdd70693,
    0x54de5729, 0x23d967bf, 0xb3667a2e, 0xc4614ab8, 0x5d681b02, 0x2a6f2b94,
    0xb40bbe37, 0xc30c8ea1, 0x5a05df1b, 0x2d02ef8d,
];
