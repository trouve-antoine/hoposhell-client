use std::os::unix::prelude::MetadataExt;
use serde_json;

use crate::{commands::{file_list::{FileInfos, FileType}, command_error::make_error}, constants::OutputFormat};

use super::{request_or_response::{maybe_string, Request, make_shell_target}, file_list::print_file_list};

pub const COMMAND_NAME: &str = "ls";


pub fn process_ls_command(
    payload: &[u8],
) -> Result<serde_json::Value, serde_json::Value> {
    let folder_path = maybe_string(Some(payload));

    if folder_path.is_none() {
        return Result::Err(make_error("No folder path provided"));
    }

    let folder_path = folder_path.unwrap();
    let folder_path = String::from(shellexpand::tilde(folder_path.as_str()));
    let entries = std::fs::read_dir(&folder_path);

    if entries.is_err() {
        let error = if let Some(err) = entries.err() { err } else {
            std::io::Error::new(std::io::ErrorKind::Other, "Unknown error")
        };
        return Result::Err(make_error(format!("Cannot list folder {}: {}", &folder_path, error.to_string()).as_str()));
    }

    let entries = entries.unwrap();

    eprintln!("Now listing files in folder: {}", &folder_path);

    let mut files = vec![];
    for entry in entries {
        if let Ok(entry) = entry {
            match entry.metadata() {
                Ok(infos) => {
                    files.push(FileInfos {
                        name: entry.file_name().into_string().unwrap(),
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
                }
            }
        }
    }
    return Result::Ok(serde_json::json!({
        "entries": files
    }));
}

pub fn process_ls_response(response_payload: &[u8], format: OutputFormat) {
    print_file_list(response_payload, format);
}

pub fn make_ls_request(make_id: impl Fn() -> String, shell_id: &String, folder_path: &String) -> Request {
    let payload = folder_path.clone().into_bytes();
    return Request {
        cmd: "ls".to_string(),
        message_id: make_id(),
        target: make_shell_target(shell_id),
        payload
    }
}