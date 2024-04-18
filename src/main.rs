use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

type Term = String;
type DocumentId = String;
type IndexType = HashMap<Term, HashSet<DocumentId>>;

#[derive(Debug, Serialize, Deserialize)]
struct FileData {
    /// name of the zip archive
    name: String,
    /// list of files in the zip archive
    files: Vec<String>,
}

fn load_data(data_filename: &str) -> Result<IndexType, Box<dyn std::error::Error>> {
    let file = File::open(data_filename)?;
    let reader = BufReader::new(file);

    let mut index = IndexType::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        let file_data: FileData = serde_json::from_str(line)?;
        for path in file_data.files.iter() {
            for term in path.split("/") {
                // index
                //     .entry(term.to_string())
                //     .or_insert_with(HashSet::new)
                //     .insert(file_data.name.clone());
                if !index.contains_key(term) {
                    index.insert(term.to_string(), HashSet::new());
                }
                if let Some(set) = index.get_mut(term) {
                    set.insert(file_data.name.clone());
                }
            }
        }
    }

    Ok(index)
}

fn run_search(data: &HashMap<String, HashSet<String>>, search: Vec<String>) {
    let mut search_results: HashMap<DocumentId, u64> = HashMap::new();
    for term in search.iter() {
        if data.contains_key(term) {
            for doc_id in data[term].iter() {
                if !search_results.contains_key(doc_id) {
                    search_results.insert(doc_id.clone(), 0);
                }
                *search_results.get_mut(doc_id).unwrap() += 1;
            }
        }
    }

    let mut search_results: Vec<(String, u64)> = search_results
        .iter()
        .map(|(doc_id, app)| (doc_id.to_string(), app.clone()))
        .collect();
    search_results.sort_by(|a, b| b.1.cmp(&a.1));

    for res in search_results.iter() {
        println!("In {} found {}/{} terms", res.0, res.1, search.len());
    }
    println!("Found {} matches", search_results.len());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let start = Instant::now();
    let data_filename = &args[1];
    let data = load_data(&data_filename)?;
    println!("loaded data for {} terms", data.len());
    println!("elapsed time: {:?}", start.elapsed());

    let search: Vec<String> = vec![
        "lombok".to_string(),
        "AUTHORS".to_string(),
        // "README.md".to_string(),
    ];

    let start = Instant::now();
    run_search(&data, search);
    println!("elapsed time: {:?}", start.elapsed());

    Ok(())
}
