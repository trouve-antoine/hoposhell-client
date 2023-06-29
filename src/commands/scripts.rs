use std::{io::Write};

use crate::constants::{OutputFormat};
use super::{request_or_response::{Request, make_shell_target, maybe_string}, command_error::make_error_bytes};

pub const COMMAND_NAME: &str = "scripts";

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct ScriptsCommandRequestBody {
    name: String
}

pub fn make_scripts_request(make_id: impl Fn() -> String, shell_id: &String, name: String) -> Request{
    let scripts_request = ScriptsCommandRequestBody { name };

    let payload = serde_json::to_vec(&scripts_request).unwrap();

    return Request {
        cmd: COMMAND_NAME.to_string(),
        message_id: make_id(),
        target: make_shell_target(shell_id),
        payload
    };
}

pub fn process_scripts_command(
    payload: &[u8]
) -> Result<Vec<u8>, Vec<u8>> {
    let text_payload = maybe_string(Some(payload));
    if text_payload.is_none() {
        return Result::Err(make_error_bytes("No text payload provided"));
    }
    let text_payload = text_payload.unwrap();
    
    /* Decode and validate the text payload to json */
    let json_payload = serde_json::from_str::<ScriptsCommandRequestBody>(text_payload.as_str());
    if json_payload.is_err() {
        return Result::Err(make_error_bytes(format!("Invalid json payload: {}", json_payload.err().unwrap().to_string()).as_str()));
    }
    let request_infos = json_payload.unwrap();

    /* Execute script and get stdout */
    let script_cmd = std::process::Command::new("ls").output();
    if let Err(e) = script_cmd {
        return Result::Err(make_error_bytes(format!("Error while executing script: {}", e.to_string()).as_str()));
    }
    let script_cmd = script_cmd.unwrap();
    let mut output = script_cmd.stdout;
    let code = script_cmd.status.code().unwrap_or_else(|| {
        if script_cmd.status.success() { 0 } else { 1 }
    });
    let mut response = vec![];
    response.append(&mut code.to_ne_bytes().to_vec());
    response.append(&mut output);

    return Result::Ok(response);

}

pub fn process_script_response(response_payload: &[u8], format: OutputFormat) {
    if response_payload.len() < 4 {
        eprintln!("Invalid response payload");
        std::process::exit(-1);
    }
    let mut code_bytes = [0u8; 4];
    code_bytes.copy_from_slice(&response_payload[0..4]);
    let code = i32::from_ne_bytes(code_bytes);

    if code != 0 {
        eprintln!("Script exited with code {}", code);
    }
    
    let script_output = &response_payload[4..];
    
    match format {
        OutputFormat::Raw => {
            std::io::stdout().write_all(script_output).unwrap();
        },
        OutputFormat::Text => {
            let script_output_text = String::from_utf8(script_output.to_vec()).unwrap();
            println!("{}", script_output_text);
        },
        OutputFormat::Json => {
            let script_output_text = String::from_utf8(script_output.to_vec()).unwrap();
            println!("{}", serde_json::to_string(&script_output_text).unwrap());
        }
    }
}