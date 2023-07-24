/**
 * hopo command <shell_id> download <remote_file_path> [local_file_path]
 * hopo command <shell_id> download <remote_file_path> -
 */


use std::{path::Path, io::Write};

use super::{request_or_response::{maybe_string, Request, make_shell_target}, command_error::make_error_bytes};

pub const COMMAND_NAME: &str = "download";
pub const COMMAND_ALIAS: &str = "cp";

pub fn process_download_command(
    payload: &[u8],
) -> Result<Vec<u8>, Vec<u8>> {
    let file_path = maybe_string(Some(payload));

    if file_path.is_none() {
        return Result::Err(make_error_bytes("No file path provided"));
    }

    let file_path = file_path.unwrap();

    let file_path = String::from(shellexpand::tilde(file_path.as_str()));

    let file_path = std::path::Path::new(&file_path);

    if !file_path.is_file() {
        return Result::Err(make_error_bytes(format!("File {} does not exist", file_path.to_str().unwrap()).as_str()));
    }

    let file_contents = std::fs::read(file_path);

    return match file_contents {
        Ok(file_contents) => Result::Ok(file_contents),
        Err(_) => Result::Err(make_error_bytes(format!("Cannot read file {}", file_path.to_str().unwrap()).as_str()))
    }
}

pub fn process_download_response(response_payload: &[u8], remote_file_path: &String, local_file_path: Option<String>) {
    match local_file_path.as_deref() {
        Some("-") => {
            std::io::stdout().write(response_payload).unwrap();
            return;
        },
        _ => {
            /* Do nothing */
        }
    }
    
    let target_path = compute_destination(&remote_file_path, local_file_path);
    if target_path.is_none() {
        eprintln!("Invalid destination path");
        std::process::exit(-1);
    }
    let target_path = target_path.unwrap();


    match std::fs::write(&target_path, response_payload) {
        Ok(_) => {
            eprintln!("Downloaded file to {}", target_path);
        },
        Err(_) => {
            eprintln!("Failed to write file to {}", target_path);
            return;
        }
    }
}

pub fn make_download_request(make_id: impl Fn() -> String, shell_id: &String, file_path: &String) -> Request {
    let payload = file_path.clone().into_bytes();
    return Request {
        cmd: "download".to_string(),
        message_id: make_id(),
        target: make_shell_target(shell_id),
        payload
    }
}

pub fn compute_destination(remote_file_path: &String, dst_path: Option<String>) -> Option<String> {
    let dst_path = match dst_path {
        Some(dst_path) => dst_path,
        None => String::from("./")
    };

    let file_name = Path::new(&remote_file_path).file_name().unwrap();
    
    // let mut dst_path = Path::new(&dst_path).canonicalize().unwrap();
    let mut dst_path = Path::new(&dst_path).to_path_buf();

    if dst_path.is_dir() {
        /* The (un)specified path is an existing folder: will add the file inside it */
        dst_path = dst_path.join(&file_name);
    } else if dst_path.is_file() {
        /* The destination path is an existing file */
        eprintln!("I will override file {}", dst_path.to_str().unwrap());
    } else {
        /* The destination path does not exist */
        // let is_dir = dst_path.ends_with(format!("{}", std::path::MAIN_SEPARATOR));
        let is_dir = dst_path.as_os_str().to_str().unwrap().ends_with(std::path::MAIN_SEPARATOR);

        if is_dir {
            /* The destination is a non existing folder: we don't create folders automatically */
            eprintln!("The target path is invalid: it is a non-existing folder");
            return None
        }

        let parent = dst_path.parent();
        if let None = parent {
            eprintln!("The target path is invalid: it is not an existing file or folder, and has no parent folder");
            return None
        }

        let parent = parent.unwrap();
        if !parent.is_dir() {
            eprintln!("The target path is invalid: it is inside a non-existing folder: {}", parent.to_str().unwrap());
            return None
        }
    }

    return Some(String::from(dst_path.to_str().unwrap()));
}