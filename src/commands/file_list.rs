use std::os::unix::prelude::MetadataExt;

use std::{fmt::Debug};
use serde::{Serialize, Deserialize};

use crate::constants::OutputFormat;

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

impl FileInfos {
    pub fn from_metadata(metadata: std::fs::Metadata, file_name: String) -> FileInfos {
        FileInfos {
            name: file_name,
            file_type: if metadata.is_dir() { FileType::Folder } else { FileType::File },
            creation_timestamp: match metadata.created() {
                Ok(created) => created.duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                Err(_) => 0
            },
            modification_timestamp: match metadata.modified() {
                Ok(modified) => modified.duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                Err(_) => 0
            },
            size_in_bytes: metadata.size()
        }
    }
}

pub fn print_file_list(response_payload: &[u8], format: OutputFormat) {
    let response_payload_json: serde_json::Value = match serde_json::from_slice(response_payload) {
        Ok(response_payload_json) => response_payload_json,
        Err(_) => {
            eprintln!("Failed to parse ls response");
            eprintln!("{}", String::from_utf8(response_payload.to_vec()).unwrap());
            return;
        }
    };
    let files = response_payload_json["entries"].as_array().unwrap();

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&files).unwrap());
        },
        _ => {
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
    }
}