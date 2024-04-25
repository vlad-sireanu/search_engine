use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    time::Instant,
};

use serde::{Deserialize, Serialize};

type Term = String;
type DocumentId = String;

const K1: f64 = 1.6;
const B: f64 = 0.75;

struct TermData {
    term_docs: HashMap<DocumentId, u64>,
    idf: f64,
}

struct IndexedData {
    term_data: HashMap<Term, TermData>,
    doc_len: HashMap<DocumentId, u64>,
    avgdl: f64,
}

impl IndexedData {
    fn new() -> Self {
        Self {
            term_data: HashMap::new(),
            doc_len: HashMap::new(),
            avgdl: 0.0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FileData {
    // name of the zip archive
    name: DocumentId,
    // list of files in the zip archive
    files: Vec<String>,
}

fn load_data(data_filename: &str) -> Result<IndexedData, Box<dyn std::error::Error>> {
    let file = File::open(data_filename)?;
    let reader = BufReader::new(file);

    let mut index = IndexedData::new();

    let mut n: i32 = 0;

    for line in reader.lines() {
        n += 1;
        let line = line?;

        let fd: FileData = serde_json::from_str(&line)?;
        let mut doc_len = 0;
        for file in fd.files {
            for term in file.split("/") {
                doc_len += 1;
                // index
                //     .entry(term.to_string())
                //     .or_insert(HashSet::new())
                //     .insert(fd.name.clone());
                if let Some(td) = index.term_data.get_mut(term) {
                    // if let Some(app) = td.term_docs.get_mut(&fd.name) {
                    //     *app += 1;
                    // } else {
                    //     td.term_docs.insert(fd.name.clone(), 1);
                    // }
                    let x = td.term_docs.entry(fd.name.clone()).or_insert(0);
                    *x += 1;
                } else {
                    let mut map = HashMap::new();
                    map.insert(fd.name.clone(), 1);
                    let td = TermData {
                        term_docs: map,
                        idf: 0.0,
                    };
                    index.term_data.insert(term.to_string(), td);
                }
            }
        }
        index.doc_len.insert(fd.name, doc_len);
        index.avgdl += doc_len as f64;
    }
    index.avgdl /= n as f64;

    for (_term, entries) in index.term_data.iter_mut() {
        let nq = entries.term_docs.len() as f64;
        entries.idf = (((n as f64 - nq + 0.5) / (nq + 0.5)) + 1.0).ln();
    }

    Ok(index)
}

fn run_search(data: &IndexedData, terms: Vec<&str>) -> Vec<(DocumentId, f64)> {
    let mut counter: HashMap<DocumentId, f64> = HashMap::new();
    for term in &terms {
        if let Some(td) = data.term_data.get(*term) {
            for (doc, app) in td.term_docs.iter() {
                let x = counter.entry(doc.to_string()).or_insert(0.0);
                *x += td.idf * (*app as f64 * (K1 + 1.0))
                    / (*app as f64 + K1 * (1.0 - B + B * data.doc_len[doc] as f64 / data.avgdl));
            }
        }
    }

    let mut scores: Vec<(DocumentId, f64)> = Vec::new();
    for (doc, cnt) in counter {
        scores.push((doc.clone(), cnt));
    }
    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("{:?}", scores);
    scores
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let data_filename = &args[1];

    let start = Instant::now();
    let data = load_data(&data_filename)?;
    println!("loaded data for {} terms", data.term_data.len());
    println!("elapsed time: {:?}", start.elapsed());

    let pair_count = data
        .term_data
        .iter()
        .map(|(_, td)| td.term_docs.len())
        .sum::<usize>();
    println!("there are {} term-docid pairs", pair_count);

    let start = Instant::now();
    let search = vec!["lombok", "AUTHORS", "README.md"];
    run_search(&data, search);
    println!("search took: {:?}", start.elapsed());
    Ok(())
}
