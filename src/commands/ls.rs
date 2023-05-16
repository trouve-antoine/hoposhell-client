use std::fmt::Debug;
use serde::{Serialize, Deserialize};
use serde_json;

use super::request_or_response::maybe_string;

#[derive(Serialize, Deserialize, Debug)]
pub enum FileType {
    File,
    Folder
}

#[derive(Serialize, Deserialize)]
pub struct FileInfos {
    pub name: String,
    pub file_type: FileType,
    pub creation_timestamp: u64,
    pub modification_timestamp: u64,
    pub size_in_bytes: u64
}

pub fn process_ls_command(
    payload: &[u8],
) -> Option<serde_json::Value> {
    let folder_path = maybe_string(Some(payload));

    match folder_path {
        Some(folder_path) => match std::fs::read_dir(folder_path) {
            Ok(entries) => {
                let mut files = vec![];
                for entry in entries {
                    if let Ok(entry) = entry {
                        match entry.metadata() {
                            Ok(infos) => {
                                files.push(FileInfos {
                                    name: entry.file_name().into_string().unwrap(),
                                    file_type: if infos.is_dir() { FileType::Folder } else { FileType::File },
                                    creation_timestamp: infos.created().unwrap().duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                                    modification_timestamp: infos.modified().unwrap().duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                                    size_in_bytes: infos.len()
                                });
                            },
                            Err(_) => {
                                /* Cannot get file infos: ignore */
                            }
                        }
                    }
                }
                return Some(serde_json::json!({
                    "files": files
                }));
            },
            Err(_) => {
                return None;
            }
        },
        None => {
            return None;
        }
    }
}

pub fn process_ls_response(response_payload: &[u8]) {
    let response_payload_json: serde_json::Value = serde_json::from_slice(response_payload).unwrap();
    let files = response_payload_json["files"].as_array().unwrap();
    
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