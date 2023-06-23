use std::{io::{Write, Read}, time::Duration};

use crate::constants::{OutputFormat};
use super::{request_or_response::{Request, make_shell_target, maybe_string}, command_error::make_error_bytes};

pub const COMMAND_NAME: &str = "tcp";

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct TcpCommandRequestBody {
    host: String,
    port: u16,
    payload: Vec<u8>
}

pub fn make_tcp_request(make_id: impl Fn() -> String, shell_id: &String, host: String, port: u16, payload: Vec<u8>) -> Request{
    let tcp_request = TcpCommandRequestBody { host, port, payload };

    let payload = serde_json::to_vec(&tcp_request).unwrap();

    return Request {
        cmd: COMMAND_NAME.to_string(),
        message_id: make_id(),
        target: make_shell_target(shell_id),
        payload
    };
}

pub fn process_tcp_command(
    payload: &[u8]
) -> Result<Vec<u8>, Vec<u8>> {
    let text_payload = maybe_string(Some(payload));
    if text_payload.is_none() {
        return Result::Err(make_error_bytes("No text payload provided"));
    }
    let text_payload = text_payload.unwrap();

    /* Decode and validate the text payload to json */
    let json_payload = serde_json::from_str::<TcpCommandRequestBody>(text_payload.as_str());
    if json_payload.is_err() {
        return Result::Err(make_error_bytes(format!("Invalid json payload: {}", json_payload.err().unwrap().to_string()).as_str()));
    }
    let request_infos = json_payload.unwrap();

    /* Make TCP request at specified location */
    let mut stream = std::net::TcpStream::connect(format!("{}:{}", request_infos.host, request_infos.port)).unwrap();
    let write_res = stream.write(&request_infos.payload);

    /* Make sure write succeeded and the correct amount of bytes was written */
    if write_res.is_err() {
        return Result::Err(make_error_bytes(format!("Error while writing to tcp stream: {}", write_res.err().unwrap().to_string()).as_str()));
    }
    let write_res = write_res.unwrap();

    if write_res != request_infos.payload.len() {
        return Result::Err(make_error_bytes(format!("Wrong number of bytes written to tcp steam: {} bytes written instead of {} bytes", write_res, request_infos.payload.len()).as_str()));
    }

    /* Read response from stream */
    stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    
    let mut response = vec![];
    let read_res = stream.read_to_end(&mut response);
    
    // let mut response = vec![0; crate::constants::BUF_SIZE];
    // let read_res = stream.read(&mut response);

    if read_res.is_err() {
        return Result::Err(make_error_bytes(format!("Error while reading from tcp stream: {}", read_res.err().unwrap().to_string()).as_str()));
    }

    return Result::Ok(response);

}

pub fn process_tcp_response(response_payload: &[u8], format: OutputFormat) {
    /* The payload is the raw TCP response */
    match format {
        OutputFormat::Raw => {
            std::io::stdout().write_all(response_payload).unwrap();
        },
        OutputFormat::Text => {
            let response_text = String::from_utf8(response_payload.to_vec()).unwrap();
            println!("{}", response_text);
        },
        OutputFormat::Json => {
            let response_text = String::from_utf8(response_payload.to_vec()).unwrap();
            println!("{}", serde_json::to_string(&response_text).unwrap());
        }
    }
}