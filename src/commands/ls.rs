use serde_json;

use crate::{commands::{file_list::FileInfos, command_error::make_error}, constants::OutputFormat};

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

    let glob_res = glob::glob(folder_path.as_str());
    if glob_res.is_err() {
        return Result::Err(make_error(format!("Invalid pattern {}: {}", &folder_path, glob_res.err().unwrap().to_string()).as_str()));
    }

    let glob_res = glob_res.unwrap();

    let mut files = vec![];
    for entry in glob_res {
        match entry {
            Ok(path) => {
                match path.metadata() {
                    Ok(infos) => {
                        files.push(FileInfos::from_metadata(infos, path.into_os_string().into_string().unwrap()));  
                    },
                    Err(_) => {
                        /* Cannot get file infos: ignore */
                    }
                }
            },
            Err(_) => {
                eprintln!("Got invalid glob entry: {:?}", entry)
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