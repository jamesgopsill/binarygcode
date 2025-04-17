use core::str;

use crate::common::{BinaryGcodeError, BlockKind, Checksum, CompressionAlgorithm, Encoding};
use crate::deserialiser::{DeserialisedResult, Deserialiser};
use crate::serialiser::{serialise_block, serialise_file_header};
use alloc::string::ToString;
use alloc::{borrow::ToOwned, boxed::Box, vec::Vec};
use base64::prelude::BASE64_STANDARD;
use base64::{encode, Engine};
use regex::Regex;

/// Provide a reference to a u8 slice of the entire binary file
/// you would like to decode.
pub fn binary_to_ascii(binary: &[u8]) -> Result<Box<str>, BinaryGcodeError> {
	let mut out = Vec::new();
	let mut deserialiser = Deserialiser::default();
	deserialiser.digest(binary);

	// Loop through running deserialise on the deserialisers inner
	// buffer with it returning either a header, block or request for more bytes.
	// Or an error when deserialising.
	loop {
		let r = deserialiser.deserialise()?;
		match r {
			DeserialisedResult::FileHeader(_) => {}
			DeserialisedResult::Block(mut b) => {
				b.to_ascii(&mut out)?;
			}
			DeserialisedResult::MoreBytesRequired(_) => {
				break;
			}
		}
	}

	let gcode = str::from_utf8(&out).unwrap().to_owned().into_boxed_str();
	Ok(gcode)
}

/// Returns a bgocde from an ascii binary
///
/// Notes:
/// Maintains the comments `;` in the metadata lines that duplicates
/// on the deserialise. Could add a check if they exist on the deserialise side
/// and add them if not. And need to remove them on this side to save space??
pub fn ascii_to_binary(ascii: &str) -> Result<Box<[u8]>, BinaryGcodeError> {
	let mut binary: Vec<u8> = Vec::new();
	let header = serialise_file_header(1, Checksum::Crc32);
	binary.extend(header);

	// Find thumbnails
	// TODO: encode them.
	let mut inner = ascii;
	while let Some(start) = inner.find("thumbnail begin") {
		if let Some(end) = inner[start..].find("; thumbnail end") {
			//let mut s = inner[..end].to_string();
			//let (left, right) = s.sp
			//let (left, right) = inner[..end].split_once(";").unwrap();

			/*
			s = s.replace("\n", "");
			s = s.replace(";", "");
			let s = s.trim();
			let r = BASE64_STANDARD.decode(s);
			if r.is_err() {
				return Err(BinaryGcodeError::SerialiseError);
			}

			// let r = r.unwrap();
			let block = serialise_block(
				BlockKind::FileMetadata,
				CompressionAlgorithm::None,
				Encoding::INI,
				Checksum::Crc32,
				line.as_bytes(),
			)?;
			binary.extend(block);
			*/

			// TODO: pack the thumbnail block;
			inner = &inner[end..];
		} else {
			return Err(BinaryGcodeError::SerialiseError);
		}
	}

	// File metadata
	for line in ascii.lines() {
		if line.starts_with("; generated by") {
			let block = serialise_block(
				BlockKind::FileMetadata,
				CompressionAlgorithm::None,
				Encoding::INI,
				Checksum::Crc32,
				line.as_bytes(),
			)?;
			binary.extend(block);
			break;
		}
	}

	// Printer Metadata
	if let Some(start) = ascii.find("; printer_model") {
		let needle = "\n\n";
		if let Some(end) = ascii[start..].find(needle) {
			let block_data = &ascii[start..start + end + needle.len()];
			let block = serialise_block(
				BlockKind::PrinterMetadata,
				CompressionAlgorithm::None,
				Encoding::INI,
				Checksum::Crc32,
				block_data.as_bytes(),
			)?;
			binary.extend(block);
		} else {
			return Err(BinaryGcodeError::SerialiseError);
		}
	}

	// Slicer config (prusa slicer only atm)
	if let Some(start) = ascii.find("; prusaslicer_config = begin") {
		let needle = "; prusaslicer_config = end";
		if let Some(end) = ascii[start..].find(needle) {
			let block_data = &ascii[start..start + end + needle.len()];
			let block = serialise_block(
				BlockKind::SlicerMetadata,
				CompressionAlgorithm::Deflate,
				Encoding::INI,
				Checksum::Crc32,
				block_data.as_bytes(),
			)?;
			binary.extend(block);
		} else {
			return Err(BinaryGcodeError::SerialiseError);
		}
	}

	// Gcode
	if let Some(start) = ascii.find("M73 P0") {
		let needle = "M73 P100 R0\n";
		if let Some(end) = ascii[start..].find(needle) {
			let gcode = &ascii[start..start + end + needle.len()];
			// Need to chunk it up to account for the u16 slice input buffer.
			let mut chunk: Vec<u8> = Vec::new();
			for b in gcode.as_bytes() {
				chunk.push(*b);
				// If the chunk is nearing max u16 and
				// we reach a new line then encode it.
				// TODO: decide what is a reasonable size gcode chunk
				// and check against the libgcode reference.
				if u16::MAX - (chunk.len() as u16) < 100 && *b == 10 {
					let block = serialise_block(
						BlockKind::GCode,
						CompressionAlgorithm::Heatshrink11_4,
						Encoding::ASCII,
						Checksum::Crc32,
						&chunk,
					)?;
					binary.extend(block);
					chunk.clear();
				}
			}

			// One remaining chunk
			if !chunk.is_empty() {
				let block = serialise_block(
					BlockKind::GCode,
					CompressionAlgorithm::Heatshrink11_4,
					Encoding::ASCII,
					Checksum::Crc32,
					&chunk,
				)?;
				binary.extend(block);
				chunk.clear();
			}
		}
	}

	Ok(binary.into_boxed_slice())
}

fn thumbnail_block(thumb: &str) -> Result<Box<[u8]>, BinaryGcodeError> {
	// TODO: Add checks to the &str input

	let (left, right) = thumb.split_once(";").unwrap();

	// Left is the header and will be used to construct
	// the parameter bytes that come before the body.
	let mut encoding = Encoding::PNG;
	if left.contains("_QOI") {
		encoding = Encoding::QOI;
	} else if left.contains("_JPG") {
		encoding = Encoding::JPG;
	}

	let re = Regex::new(r"\s\d+x\d+\s").unwrap();
	let m = re.find(left);
	if m.is_none() {
		return Err(BinaryGcodeError::SerialiseError);
	}
	let m = m.unwrap().as_str();
	let (w, h) = m.split_once("x").unwrap();
	let w = w.parse::<u16>();
	if w.is_err() {
		return Err(BinaryGcodeError::SerialiseError);
	}
	let w = w.unwrap();
	let h = h.parse::<u16>();
	if h.is_err() {
		return Err(BinaryGcodeError::SerialiseError);
	}
	let h = h.unwrap();

	let mut parameters: Vec<u8> = Vec::new();
	// parameters beyond the encoding
	parameters.extend(w.to_le_bytes());
	parameters.extend(h.to_le_bytes());

	let mut right = right.to_string();
	right = right.replace("\n;", "");
	let right = right.trim();
	let data = BASE64_STANDARD.decode(right);
	if data.is_err() {
		return Err(BinaryGcodeError::SerialiseError);
	}
	let data = data.unwrap();

	serialise_block(
		BlockKind::Thumbnail,
		CompressionAlgorithm::None,
		encoding,
		Checksum::Crc32,
		&parameters,
		&data,
	)
}

mod tests {

	#[test]
	fn convert_thumbnail_block() {}
}
