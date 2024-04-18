use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct FileData {
    /// name of the zip archive
    name: String,
    /// list of files in the zip archive
    files: Vec<String>,
}

fn load_data(data_filename: &str) -> Result<Vec<FileData>, Box<dyn std::error::Error>> {
    let file = File::open(data_filename)?;
    let reader = BufReader::new(file);

    let mut data = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        data.push(serde_json::from_str(line)?);
    }

    Ok(data)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let data_filename = &args[1];
    let data = load_data(&data_filename)?;
    println!("loaded data for {} files", data.len());

    Ok(())
}
