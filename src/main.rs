use binarygcode::{ascii_to_binary, binary_to_ascii};
use clap::Parser;
use std::{fs, path::PathBuf};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    path: PathBuf,
}

pub fn main() {
    println!("BinaryGcode");
    let args = Args::parse();
    println!("{}", args.path.to_str().unwrap());

    if !args.path.exists() {
        eprintln!("File Not Found");
        return;
    }

    if !args.path.is_file() {
        eprintln!("Path is not a file");
        return;
    }

    let ext = args.path.extension();
    if ext.is_none() {
        eprintln!("File type not supported. Expecting .gcode or .bgcode.")
    }
    let ext = ext.unwrap();

    match ext.to_str().unwrap() {
        "gcode" => {
            println!("ASCII gcode -> Binary gcode");
            let gcode = fs::read_to_string(&args.path).unwrap();
            let bgcode = ascii_to_binary(&gcode).unwrap();
            let compression_ratio =
                (bgcode.len() as f64 / gcode.len() as f64) * 100.0;
            println!(
                "{} bytes -> {} bytes ({:.2}%)",
                gcode.len(),
                bgcode.len(),
                compression_ratio
            );
            let path = args.path.clone().with_extension("bgcode");
            if fs::write(path, bgcode).is_err() {
                eprintln!("Error writing file.");
                return;
            }
            println!("Conversion Complete");
        }
        "bgcode" => {
            println!("Binary gcode -> ASCII gcode");
            let bgcode = fs::read(&args.path).unwrap();
            let gcode = binary_to_ascii(&bgcode, false).unwrap();
            println!("{} bytes -> {} bytes", bgcode.len(), gcode.len(),);
            let path = args.path.clone().with_extension("gcode");
            if fs::write(path, gcode.as_bytes()).is_err() {
                eprintln!("Error writing file.");
                return;
            }
            println!("Conversion Complete");
        }
        _ => {
            eprintln!("File type note supported. Expecting .gcode or .bgcode.")
        }
    }
}
