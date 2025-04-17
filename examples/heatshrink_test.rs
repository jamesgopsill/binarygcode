use core::panic;
use std::{env, fs, iter::zip};

use embedded_heatshrink::{
	HSDFinishRes, HSDPollRes, HSDSinkRes, HSEFinishRes, HSEPollRes, HSESinkRes, HeatshrinkDecoder,
	HeatshrinkEncoder,
};

fn main() {
	let mut path = env::current_dir().unwrap();
	path.push("tmp");
	path.push("just_gcode_2.gcode");

	let data = fs::read(path).unwrap();
	// Will do it as chunks in the future.
	let data = &data[..u16::MAX as usize];

	let mut encoder = HeatshrinkEncoder::new(11, 4).unwrap();
	let mut sunk: usize = 0;
	let mut polled: usize = 0;

	let mut compressed = vec![0u8; data.len()];
	while sunk < data.len() {
		println!("LOOPING");
		match encoder.sink(&data[sunk..]) {
			HSESinkRes::Ok(sz) => {
				sunk += sz;
			}
			_ => panic!("Logic Error"),
		}
		loop {
			match encoder.poll(&mut compressed[polled..]) {
				HSEPollRes::Empty(sz) => {
					println!("EMPTY {}", sz);
					polled += sz;
					if sz == 0 {
						break;
					}
				}
				// This is never called. Curious. I thought it would
				// call this many times and empty once but empty is being
				// called many times and the last time.
				HSEPollRes::More(sz) => {
					println!("MORE {}", sz);
					polled += sz;
				}
				_ => panic!("Logic Error"),
			}
		}
	}

	loop {
		match encoder.finish() {
			HSEFinishRes::Done => {
				println!("DONE");
				break;
			}
			HSEFinishRes::More => match encoder.poll(&mut compressed[polled..]) {
				// Empty is called again multiple times. Maybe more is when
				// the out_buf hasn't got sufficient size.
				HSEPollRes::Empty(sz) => {
					println!("EMPTY {}", sz);
					polled += sz;
				}
				// This is also never called.
				HSEPollRes::More(sz) => {
					println!("MORE {}", sz);
					polled += sz;
				}
				_ => panic!("Logic Error"),
			},
			_ => panic!("Logic Error"),
		}
	}

	let polled_u16 = polled as u16;
	println!("Original Data Len: {}", data.len()); //, sunk, polled, polled_u16);
	println!("Sunk: {}", sunk);
	println!("Polled: {}", polled);
	println!("Polled u16: {}", polled_u16);

	println!("{:?}", &compressed[polled - 10..polled + 10]);
	let compressed = &compressed[..polled];
	println!("{}", compressed.len());

	let mut decoder = HeatshrinkDecoder::new(polled_u16, 11, 4).unwrap();

	let mut uncompressed: Vec<u8> = vec![0; data.len()];
	polled = 0;
	sunk = 0;
	while sunk < compressed.len() {
		println!("LOOPING");
		match decoder.sink(&compressed[sunk..]) {
			HSDSinkRes::Ok(sz) => {
				sunk += sz;
			}
			HSDSinkRes::Full => panic!("HSDSinkRes::Full"),
			HSDSinkRes::ErrorNull => panic!("HSDSinkRes::ErrorNull"),
		}
		loop {
			println!("Looping again");
			let res = decoder.poll(&mut uncompressed[polled..]);
			match res {
				HSDPollRes::Empty(sz) => {
					println!("EMPTY {}", sz);
					polled += sz;
					if sz == 0 {
						break;
					}
				}
				// Panics after looping for more. Is there a bug where
				// More is Empty and Empty is More?
				HSDPollRes::More(sz) => {
					println!("MORE {}", sz);
					polled += sz;
					break;
				}
				HSDPollRes::ErrorUnknown => panic!("HSDPollRes::ErrorUnknown"),
				HSDPollRes::ErrorNull => panic!("HSDPollRes::ErrorNull"),
			}
			println!("GOT HERE")
		}
	}

	println!("FINISH");

	loop {
		match decoder.finish() {
			HSDFinishRes::Done => {
				println!("DONE");
				break;
			}
			HSDFinishRes::More => match decoder.poll(&mut uncompressed[polled..]) {
				HSDPollRes::Empty(sz) => {
					println!("EMPTY {}", sz);
					polled += sz;
					if sz == 0 {
						break;
					}
				}
				HSDPollRes::More(sz) => {
					println!("MORE {}", sz);
					polled += sz;
				}
				_ => panic!("Logic Error"),
			},
			HSDFinishRes::ErrorNull => panic!("ErrorNull"),
		}
	}

	println!("Decoder Sunk: {}, Polled: {}", sunk, polled);

	for (idx, (a, b)) in zip(data, uncompressed).enumerate() {
		if *a != b {
			println!("Different at {}", idx);
			break;
		}
	}

	println!("MATCHY MATCHY")

	//let data = str::from_utf8(&data).unwrap();
	//println!("{}", &data[data.len() - 30..]);
	//let uncompressed = &uncompressed[..polled];
	//let uncompressed = str::from_utf8(uncompressed).unwrap();
	//println!("{}", &uncompressed[uncompressed.len() - 30..]);

	//println!("{:?}", &data[..100]);
	//println!("{:?}", &uncompressed[..100]);

	//println!("{:?}", &data[data.len() - 1_000..]);
	//println!("{:?}", &uncompressed[uncompressed.len() - 1_000..]);

	//println!("{} {}", data.len(), uncompressed.len());
}
