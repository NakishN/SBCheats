// src/sync.rs
use crate::{CheatInfo};
use csv::StringRecord;
use reqwest::blocking::Client;
use serde_json::json;
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Message to UI log channel when DB sync happens
pub const SYNC_MSG_PREFIX: &str = "📡 Sheets sync:";

/// Start a background thread that periodically fetches the public CSV export of the Google Sheet,
/// parses it into a HashMap<String, CheatInfo> and replaces the shared_db contents.
///
/// - csv_url: direct CSV export URL (https://docs.google.com/spreadsheets/d/<ID>/export?format=csv&gid=<GID>)
/// - shared_db: Arc<Mutex<HashMap<cheat_name, CheatInfo>>> — will be replaced atomically
/// - interval_seconds: sync interval in seconds
/// - ui_sender: optional sender to post simple log messages to UI (ScanMessage::FileFound text)
pub fn spawn_sync_thread(
    csv_url: String,
    shared_db: Arc<Mutex<HashMap<String, CheatInfo>>>,
    interval_seconds: u64,
    ui_sender: Option<Sender<String>>,
) {
    thread::spawn(move || {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build http client");

        loop {
            match fetch_and_parse(&client, &csv_url) {
                Ok(new_map) => {
                    // replace DB atomically
                    if let Ok(mut db) = shared_db.lock() {
                        *db = new_map;
                        if let Some(ref s) = ui_sender {
                            let _ = s.send(format!("{} Обновлено записей: {}", SYNC_MSG_PREFIX, db.len()));
                        }
                    }
                }
                Err(err) => {
                    if let Some(ref s) = ui_sender {
                        let _ = s.send(format!("{} Ошибка: {}", SYNC_MSG_PREFIX, err));
                    }
                }
            }

            thread::sleep(Duration::from_secs(interval_seconds));
        }
    });
}

/// Fetch CSV and parse into HashMap<String, CheatInfo>
fn fetch_and_parse(
    client: &Client,
    csv_url: &str,
) -> Result<HashMap<String, CheatInfo>, String> {
    let resp = client
        .get(csv_url)
        .send()
        .map_err(|e| format!("request failed: {}", e))?
        .text()
        .map_err(|e| format!("read body failed: {}", e))?;

    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(resp.as_bytes());

    let headers = rdr
        .headers()
        .map_err(|e| format!("csv headers: {}", e))?
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let mut map = HashMap::new();

    for result in rdr.records() {
        let rec = result.map_err(|e| format!("csv record: {}", e))?;
        let info = parse_record(&headers, &rec);
        // choose key: prefer explicit "name" or "id" or description; fallback to incremented index
        let key = info
            .description
            .clone()
            .or_else(|| {
                rec.get(0).map(|s| s.to_string()).filter(|s| !s.is_empty())
            })
            .unwrap_or_else(|| format!("entry_{}", map.len() + 1));
        map.insert(key, info.into_inner());
    }

    Ok(map)
}

/// Helper: parse a CSV record into CheatInfo (wrapped in Result to allow empty description)
fn parse_record(headers: &[String], record: &StringRecord) -> std::result::Result<std::sync::Arc<CheatInfo>, ()> {
    // We'll map flexible header names to fields
    let mut directories: Vec<String> = Vec::new();
    let mut classes: Vec<String> = Vec::new();
    let mut exclude_dirs: Vec<String> = Vec::new();
    let mut sizes_kb: Vec<f32> = Vec::new();
    let mut description: Option<String> = None;
    let mut strict_mode = false;
    let mut min_conditions: usize = 2;
    let mut tags: Vec<String> = Vec::new();

    for (i, header) in headers.iter().enumerate() {
        let value = record.get(i).unwrap_or("").trim();
        if value.is_empty() {
            continue;
        }
        let key = header.to_lowercase();

        if key.contains("dir") || key.contains("package") || key.contains("path") {
            // allow separators: ; , |
            directories.extend(split_and_trim(value).into_iter());
            continue;
        }
        if key.contains("class") {
            classes.extend(split_and_trim(value).into_iter());
            continue;
        }
        if key.contains("exclude") || key.contains("ignore") {
            exclude_dirs.extend(split_and_trim(value).into_iter());
            continue;
        }
        if key.contains("size") || key.contains("weight") || key.contains("kb") {
            sizes_kb.extend(
                split_and_trim(value)
                    .into_iter()
                    .filter_map(|s| parse_f32(&s)),
            );
            continue;
        }
        if key.contains("descr") || key.contains("name") || key.contains("description") || key.contains("cheat") {
            description = Some(value.to_string());
            continue;
        }
        if key.contains("strict") || key.contains("strict_mode") {
            strict_mode = parse_bool(value);
            continue;
        }
        if key.contains("min") && key.contains("cond") || key.contains("min_conditions") {
            if let Ok(n) = value.parse::<usize>() {
                min_conditions = n;
            }
            continue;
        }
        // tags detection: columns named dll, client, hitbox, type
        if key.contains("dll") && !value.is_empty() {
            tags.push("dll".to_string());
            // sometimes DLL column contains size numbers — try to parse sizes too
            sizes_kb.extend(
                split_and_trim(value)
                    .into_iter()
                    .filter_map(|s| parse_f32(&s)),
            );
            continue;
        }
        if key.contains("client") {
            tags.push("client".to_string());
            continue;
        }
        if key.contains("hitbox") || key.contains("hb") {
            tags.push("hitbox".to_string());
            continue;
        }
        if key.contains("type") {
            tags.push(value.to_string());
            continue;
        }
        // fallback: if header is unknown, try to parse numbers or sizes into sizes_kb
        if let Some(n) = parse_f32(value) {
            sizes_kb.push(n);
        }
    }

    let info = CheatInfo {
        directories,
        classes,
        exclude_dirs,
        sizes_kb,
        description,
        strict_mode,
        min_conditions,
        tags,
    };

    Ok(std::sync::Arc::new(info))
}

fn split_and_trim(s: &str) -> Vec<String> {
    s.split(|c| c == ';' || c == ',' || c == '|' )
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

fn parse_f32(s: &str) -> Option<f32> {
    // remove non-digit except dot
    let cleaned: String = s.chars().filter(|c| c.is_digit(10) || *c == '.').collect();
    cleaned.parse::<f32>().ok()
}

fn parse_bool(s: &str) -> bool {
    let s = s.to_lowercase();
    matches!(s.as_str(), "1" | "true" | "yes" | "y" | "да")
}