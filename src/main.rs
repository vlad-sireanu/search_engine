use serde::{Deserialize, Serialize};
use serde_json;
use std::fs::File;
use std::io::{self, prelude::*, BufReader};

#[derive(Debug, Serialize, Deserialize)]
struct FileData {
    name: String,
    files: Vec<String>,
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let zip_data = &args[1];

    let file = File::open(zip_data)?;
    let reader = BufReader::new(file);

    let mut zip_files: Vec<FileData> = Vec::new();
    for line in reader.lines() {
        zip_files.push(serde_json::from_str(&line.unwrap()).unwrap());
    }

    println!("{:?}", zip_files);

    Ok(())
}
