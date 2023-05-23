use super::request_or_response::{maybe_string, Request, make_shell_target};

pub const COMMAND_NAME: &str = "download";

pub fn process_download_command(
    payload: &[u8],
) -> Option<Vec<u8>> {
    let file_path = maybe_string(Some(payload));

    if file_path.is_none() {
        eprintln!("No file path provided");
        return None;
    }

    let file_path = file_path.unwrap();

    let file_path = String::from(shellexpand::tilde(file_path.as_str()));

    let file_path = std::path::Path::new(&file_path);

    if !file_path.is_file() {
        return None;
    }

    let file_contents = std::fs::read(file_path);

    return match file_contents {
        Ok(file_contents) => Some(file_contents),
        Err(_) => None
    }
}

pub fn process_download_response(response_payload: &[u8], target_path: &String) {
    match std::fs::write(target_path, response_payload) {
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