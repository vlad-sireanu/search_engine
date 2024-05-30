#[macro_use]
extern crate rocket;

use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    sync::{Arc, RwLock},
    time::Instant,
};

use rocket::{fs::FileServer, serde::json::Json, State};

use serde::{Deserialize, Serialize};

type Term = String;
type DocumentId = String;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const K1: f64 = 1.6;
const B: f64 = 0.75;

#[derive(Default, Serialize, Deserialize)]
struct TermData {
    term_docs: HashMap<u32, u32>,
    idf: f64,
}

#[derive(Default, Serialize, Deserialize)]
struct IndexedData {
    id_to_docs: Vec<DocumentId>,
    term_data: HashMap<Term, TermData>,
    doc_len: HashMap<u32, u32>,
    avgdl: f64,
}

impl IndexedData {
    fn new() -> Self {
        Self {
            id_to_docs: Vec::new(),
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

fn load_data(data_filename: &str) -> eyre::Result<IndexedData> {
    let file = File::open(data_filename)?;
    let reader = BufReader::new(file);

    let mut index = IndexedData::new();

    let mut n: i32 = 0;

    for line in reader.lines() {
        n += 1;
        let line = line?;

        let fd: FileData = serde_json::from_str(&line)?;
        let mut doc_len = 0;

        let doc_id = index.id_to_docs.len() as u32;
        index.id_to_docs.push(fd.name.clone());

        for file in fd.files {
            for term in file.split('/') {
                doc_len += 1;
                if let Some(td) = index.term_data.get_mut(term) {
                    let x = td.term_docs.entry(doc_id).or_insert(0);
                    *x += 1;
                } else {
                    let mut map: HashMap<u32, u32> = HashMap::new();
                    map.insert(doc_id, 1);
                    let td = TermData {
                        term_docs: map,
                        idf: 0.0,
                    };
                    index.term_data.insert(term.to_string(), td);
                }
            }
        }
        index.doc_len.insert(doc_id, doc_len);
        index.avgdl += doc_len as f64;
    }
    index.avgdl /= n as f64;

    for (_term, entries) in index.term_data.iter_mut() {
        let nq = entries.term_docs.len() as f64;
        entries.idf = (((n as f64 - nq + 0.5) / (nq + 0.5)) + 1.0).ln();
    }

    Ok(index)
}

fn save_data(filename: &str, data: &IndexedData) -> eyre::Result<()> {
    let file = File::create(filename)?;
    use std::io::BufWriter;
    let mut wr = BufWriter::new(file);
    rmp_serde::encode::write_named(&mut wr, data)?;
    Ok(())
}

fn load_msgpack(filename: String) -> eyre::Result<IndexedData> {
    let file = File::open(filename)?;
    let rd = BufReader::new(file);
    Ok(rmp_serde::decode::from_read(rd)?)
}

fn run_search(data: &IndexedData, terms: Vec<&str>) -> Vec<(DocumentId, f64)> {
    let mut counter: HashMap<u32, f64> = HashMap::new();
    for term in terms {
        if let Some(td) = data.term_data.get(term) {
            for (doc, app) in td.term_docs.iter() {
                let x = counter.entry(*doc).or_insert(0.0);
                *x += td.idf * (*app as f64 * (K1 + 1.0))
                    / (*app as f64 + K1 * (1.0 - B + B * data.doc_len[doc] as f64 / data.avgdl));
            }
        }
    }

    let mut scores: Vec<(DocumentId, f64)> = Vec::new();
    for (doc, cnt) in counter {
        scores.push((data.id_to_docs[doc as usize].clone(), cnt));
    }
    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
    scores
}

#[derive(Serialize)]
struct Greeting {
    message: String,
}

#[derive(Serialize)]
struct SearchResult {
    matches: Vec<SearchMatch>,
    total: usize,
    time: u128,
}

#[derive(Serialize)]
struct SearchMatch {
    md5: String,
    score: f64,
}

#[derive(Debug, Deserialize)]
struct SearchData {
    terms: Vec<String>,
    max_length: Option<usize>,
    min_score: Option<f64>,
}

#[get("/")]
fn index() -> Json<Greeting> {
    Json(Greeting {
        message: "Hello, welcome to our server!".to_string(),
    })
}

#[post("/search", data = "<req>")]
fn search(
    req: Json<SearchData>,
    server_state: &State<Arc<RwLock<ServerState>>>,
) -> Result<Json<SearchResult>, String> {
    let start = Instant::now();
    let server_state = server_state
        .read()
        .map_err(|err| format!("Error: {err:#}"))?;
    let matches: Vec<SearchMatch> = run_search(
        &server_state.index,
        req.terms.iter().map(AsRef::as_ref).collect(),
    )
    .iter()
    .filter(|(_, score)| *score > req.min_score.unwrap_or(0.0))
    .take(req.max_length.unwrap_or(usize::MAX))
    .map(|(doc, score)| SearchMatch {
        md5: doc.clone(),
        score: *score,
    })
    .collect();
    Ok(Json(SearchResult {
        total: matches.len(),
        matches,
        time: start.elapsed().as_millis(),
    }))
}

use rocket::form::Form;
use rocket::fs::TempFile;

#[derive(FromForm)]
struct Upload<'r> {
    file: TempFile<'r>,
    max_length: Option<usize>,
    min_score: Option<f64>,
}

#[post("/search_by_file", data = "<upload>")]
fn search_by_file(
    upload: Form<Upload<'_>>,
    server_state: &State<Arc<RwLock<ServerState>>>,
) -> Result<Json<SearchResult>, String> {
    let start = Instant::now();
    let file = File::open(upload.file.path().unwrap()).unwrap();
    let reader = BufReader::new(file);
    let mut zip = zip::ZipArchive::new(reader).unwrap();

    let mut filenames = String::new();
    for i in 0..zip.len() {
        filenames.push_str(zip.by_index(i).unwrap().name());
        filenames.push('/');
    }
    let terms = filenames.split('/').collect::<Vec<&str>>();

    let server_state = server_state
        .read()
        .map_err(|err| format!("Error: {err:#}"))?;
    let matches: Vec<SearchMatch> = run_search(&server_state.index, terms)
        .iter()
        .filter(|(_, score)| *score > upload.min_score.filter(|x| !x.is_nan()).unwrap_or(0.0))
        .take(upload.max_length.unwrap_or(usize::MAX))
        .map(|(doc, score)| SearchMatch {
            md5: doc.clone(),
            score: *score,
        })
        .collect();
    Ok(Json(SearchResult {
        total: matches.len(),
        matches,
        time: start.elapsed().as_millis(),
    }))
}

#[derive(Default)]
struct ServerState {
    index: IndexedData,
}

#[rocket::main]
async fn main() -> eyre::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args[1] == "--save" {
        save_msgpack(&args[2], &args[3])?;
    } else {
        let start = Instant::now();
        let msg_data = load_msgpack(args[1].to_string())?;
        println!("load from masgpack in {}", start.elapsed().as_secs_f64());

        let server_state = Arc::new(RwLock::new(ServerState { index: msg_data }));
        rocket::build()
            .manage(server_state)
            .mount("/", routes![index, search, search_by_file])
            .mount("/dashboard", FileServer::from("static"))
            .ignite()
            .await?
            .launch()
            .await?;
    }

    Ok(())
}

fn save_msgpack(data_filename: &str, msgpack_file: &str) -> eyre::Result<()> {
    let start = Instant::now();
    let data = load_data(data_filename)?;
    println!("loaded data for {} terms", data.term_data.len());
    println!("elapsed time: {:?}", start.elapsed());

    let pair_count = data
        .term_data
        .values()
        .map(|td| td.term_docs.len())
        .sum::<usize>();
    println!("there are {} term-docid pairs", pair_count);

    let start = Instant::now();
    let search_args = "lombok,AUTHORS,README.md".split(',').collect();
    let matches = run_search(&data, search_args);
    println!(
        "search found {} matches in {:.2}s",
        matches.len(),
        start.elapsed().as_secs_f64(),
    );

    save_data(msgpack_file, &data)?;

    Ok(())
}
