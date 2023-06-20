use std::os::unix::prelude::MetadataExt;
use serde_json;

use crate::constants::OutputFormat;

use super::{request_or_response::{maybe_string, Request, make_shell_target}, file_list::{FileInfos, FileType, print_file_list}, command_error::make_error};

pub const COMMAND_NAME: &str = "glob";

pub fn process_glob_command(
    payload: &[u8],
) -> Result<serde_json::Value, serde_json::Value> {
    let glob_pattern = maybe_string(Some(payload));

    if glob_pattern.is_none() {
        return Result::Err(make_error("No glob_pattern path provided"));
    }

    let glob_pattern = glob_pattern.unwrap();
    let glob_pattern = String::from(shellexpand::tilde(glob_pattern.as_str()));

    let entries = glob::glob(&glob_pattern);

    if entries.is_err() {
        if let Some(err) = entries.err() {
            return Result::Err(make_error(format!("Cannot glob pattern {}: {}", &glob_pattern, err.to_string()).as_str()));
        } else {
            return Result::Err(make_error(format!("Cannot glob pattern {}: Unknown error", &glob_pattern).as_str()));
        };
    }

    let entries = entries.unwrap();

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
    return Result::Ok(serde_json::json!({
        "entries": files
    }));
}

pub fn process_glob_response(response_payload: &[u8], format: OutputFormat) {
    print_file_list(response_payload, format);
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