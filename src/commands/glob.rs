use std::{fmt::Debug, os::unix::prelude::MetadataExt};
use serde::{Serialize, Deserialize};
use serde_json;

use super::request_or_response::{maybe_string, Request, make_shell_target};

pub const COMMAND_NAME: &str = "glob";

#[derive(Serialize, Deserialize, Debug)]
pub enum FileType {
     #[serde(rename = "file")]
    File,
     #[serde(rename = "dir")]
    Folder
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfos {
    pub name: String,
    pub file_type: FileType,
    pub creation_timestamp: u64,
    pub modification_timestamp: u64,
    pub size_in_bytes: u64
}

pub fn process_glob_command(
    payload: &[u8],
) -> Option<serde_json::Value> {
    let glob_pattern = maybe_string(Some(payload));

    if glob_pattern.is_none() {
        eprintln!("No glob_pattern path provided");
        return None;
    }

    let glob_pattern = glob_pattern.unwrap();
    let glob_pattern = String::from(shellexpand::tilde(glob_pattern.as_str()));

    let entries = glob::glob(&glob_pattern);

    if entries.is_err() {
        eprintln!("Tried and failed to glob pattern: {}", &glob_pattern);
        return None;
    }

    let entries = entries.unwrap();

    println!("Now globing pattern: {}", &glob_pattern);

    let mut files = vec![];
    for entry in entries {
        if let Ok(entry) = entry {
            match entry.metadata() {
                Ok(infos) => {
                    files.push(FileInfos {
                        name: match entry.as_path().to_str() {
                            Some(path) => String::from(path),
                            None => String::from(""),
                        },
                        file_type: if infos.is_dir() { FileType::Folder } else { FileType::File },
                        creation_timestamp: match infos.created() {
                            Ok(created) => created.duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                            Err(_) => 0
                        },
                        modification_timestamp: match infos.modified() {
                            Ok(modified) => modified.duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                            Err(_) => 0
                        },
                        size_in_bytes: infos.size()
                    });
                },
                Err(_) => {
                    /* Cannot get file infos: ignore */
                    eprintln!("During glob, ignore file {:?}", entry.as_path());
                }
            }
        }
    }
    return Some(serde_json::json!({
        "entries": files
    }));
}

pub fn process_glob_response(response_payload: &[u8]) {
    let response_payload_json: serde_json::Value = match serde_json::from_slice(response_payload) {
        Ok(response_payload_json) => response_payload_json,
        Err(_) => {
            eprintln!("Failed to parse ls response");
            eprintln!("{}", String::from_utf8(response_payload.to_vec()).unwrap());
            return;
        }
    };
    let files = response_payload_json["entries"].as_array().unwrap();
    
    for file in files {
        let v = file.to_owned();
        let file = serde_json::from_value::<FileInfos>(v);

        match file {
            Ok(file) => {
                println!("{} {:?} {} {} {}", file.name, file.file_type, file.creation_timestamp, file.modification_timestamp, file.size_in_bytes);
            },
            Err(_) => {
                /* Cannot parse: ignore */
            }
        }
    }
}

pub fn make_glob_request(make_id: impl Fn() -> String, shell_id: &String, glob_pattern: &String) -> Request {
    let payload = glob_pattern.clone().into_bytes();
    return Request {
        cmd: COMMAND_NAME.to_string(),
        message_id: make_id(),
        target: make_shell_target(shell_id),
        payload
    }
}