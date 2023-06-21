use std::collections::HashMap;

use crate::constants::OutputFormat;

use super::{request_or_response::{maybe_string, make_shell_target, Request}, command_error::make_error_bytes};

pub const COMMAND_NAME: &str = "http";

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum HttpVerb {
    GET,
    POST,
    PUT,
    DELETE
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct HttpCommandRequestBody {
    verb: HttpVerb,
    url: String,
    headers: HashMap<String, String>,
    body: Option<String>
}

pub fn process_http_command(
    payload: &[u8]
) -> Result<Vec<u8>, Vec<u8>> {
    /* Decode the bytes payload to text */
    let text_payload = maybe_string(Some(payload));
    if text_payload.is_none() {
        return Result::Err(make_error_bytes("No text payload provided"));
    }
    let text_payload = text_payload.unwrap();
    
    /* Decode and validate the text payload to json */
    let json_payload = serde_json::from_str::<HttpCommandRequestBody>(text_payload.as_str());
    if json_payload.is_err() {
        return Result::Err(make_error_bytes(format!("Invalid json payload: {}", json_payload.err().unwrap().to_string()).as_str()));
    }
    let body = json_payload.unwrap();
    
    eprintln!("Got http command: {:?}", body);

    return Result::Ok(Vec::new());
}

pub fn process_http_response(response_payload: &[u8], _format: OutputFormat) {
    let payload = maybe_string(Some(response_payload));

    match payload {
        Some(payload) => {
            eprintln!("Got http response: {:?}", payload);
        },
        None => {
            eprintln!("Got invalid http response");
        }
    }
}

pub fn make_http_request(make_id: impl Fn() -> String, shell_id: &String, args: &Vec<String>) -> Request{
    let verb = args[0].clone();
    let url = args[1].clone();

    let request_body = HttpCommandRequestBody {
        verb: serde::Deserialize::deserialize(serde_json::Value::String(verb)).unwrap(),
        url,
        headers: HashMap::new(),
        body: None
    };

    let payload = serde_json::to_vec(&request_body).unwrap();

    return Request {
        cmd: "http".to_string(),
        message_id: make_id(),
        target: make_shell_target(shell_id),
        payload
    };
}