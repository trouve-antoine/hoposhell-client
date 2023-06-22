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
    let request_infos = json_payload.unwrap();
    
    let response = match request_infos.verb {
        HttpVerb::GET => {
            reqwest::blocking::get(request_infos.url.as_str())
        },
        HttpVerb::POST => {
            reqwest::blocking::Client::new().post(request_infos.url.as_str())
                .body(request_infos.body.unwrap_or("".to_string()))
                .send()
        },
        _ => {
            return Result::Err(make_error_bytes(format!("Unsupported http verb: {:?}", request_infos.verb).as_str()));
        }
    };

    match response {
        Ok(response) => {
            let contents = response.bytes();

            match contents {
                Ok(contents) => {
                    return Result::Ok(contents.to_vec());
                },
                Err(_) => {
                    return Result::Err(make_error_bytes("Cannot get response bytes"));
                }
            }
        },
        Err(_) => {
            return Result::Err(make_error_bytes("Cannot access url"));
        }
    };
}

pub fn process_http_response(response_payload: &[u8], format: OutputFormat) {
    if format == OutputFormat::Raw {
        println!("{:?}", response_payload);
        return;
    }

    let text_payload = maybe_string(Some(response_payload));

    if text_payload.is_none() {
        eprintln!("Cannot parse http payload bytes");
        return;
    }
    let text_payload = text_payload.unwrap();

    // let splitted_payload: Vec<&str> = text_payload.splitn(2, "\n\n").collect();
    // if splitted_payload.len() != 2 {
    //     eprintln!("Cannot split http payload");
    //     return;
    // }
    // let body_payload = splitted_payload[1];

    if format == OutputFormat::Text {
        println!("{}", text_payload);
        return;
    }

    if format == OutputFormat::Json {
        let body_json = serde_json::from_str::<serde_json::Value>(&text_payload);
        if body_json.is_err() {
            eprintln!("Cannot parse http payload to json");
            return;
        }
        let body_json = body_json.unwrap();
        println!("{}", body_json);
        return;
    }

    eprintln!("Unsupported output format");

}

pub fn make_http_request(make_id: impl Fn() -> String, shell_id: &String, args: &Vec<String>) -> Request{
    let verb = args[0].clone();
    let url = args[1].clone();

    let body = if args.len() > 2 { Some(args[2].clone()) } else { None };

    let request_body = HttpCommandRequestBody {
        verb: serde::Deserialize::deserialize(serde_json::Value::String(verb)).unwrap(),
        url,
        headers: HashMap::new(),
        body
    };

    let payload = serde_json::to_vec(&request_body).unwrap();

    return Request {
        cmd: "http".to_string(),
        message_id: make_id(),
        target: make_shell_target(shell_id),
        payload
    };
}