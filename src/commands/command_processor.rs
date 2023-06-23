use std::time::SystemTime;

use crate::commands::command_error::make_error_bytes;

use super::command_history::CommandHistory;
use super::request_or_response::{RequestOrResponse, Response, StatusCode};
use super::{glob, ls, download, http, tcp};

pub struct CommandProcessor {
    history: CommandHistory
}

impl CommandProcessor {
    pub fn new() -> CommandProcessor {
        return CommandProcessor {
            history: CommandHistory::new()
        }
    }

    pub fn process_msg(&mut self, msg: &Vec<u8>) -> Option<Response> {
        /* Parses and processes a command message in serialized form */
        /* (parsing is actually done inside command_history) */
        let cmd = self.history.append(msg);

        match cmd {
            RequestOrResponse::Request(req) => {
                /* Got a request from the cloud or another shell */
                /* This happens in the loop that processes incomming messages from the server */

                let response_payload = match req.cmd.as_str() {
                    ls::COMMAND_NAME => match ls::process_ls_command(&req.payload) {
                        Ok(payload) => Result::Ok(payload.to_string().as_bytes().to_vec()),
                        Err(payload) => Result::Err(payload.to_string().as_bytes().to_vec())
                    },
                    download::COMMAND_NAME => {
                        download::process_download_command(&req.payload)
                    },
                    glob::COMMAND_NAME => match glob::process_glob_command(&req.payload) {
                        Ok(payload) => Result::Ok(payload.to_string().as_bytes().to_vec()),
                        Err(payload) => Result::Err(payload.to_string().as_bytes().to_vec())
                    },
                    http::COMMAND_NAME => {
                        http::process_http_command(&req.payload)
                    },
                    tcp::COMMAND_NAME => {
                        tcp::process_tcp_command(&req.payload)
                    },
                    _ => {
                        eprintln!("[{}] Got request with unknown command: {:?}", req.message_id, req.cmd);
                        Result::Err(make_error_bytes("Unknown command"))
                    }
                };

                // let _payload = response_payload.clone().unwrap();
                // let _test = zstd::encode_all(_payload.as_slice(), 4);
                // eprintln!("ZLIB compression: {} -> {}", _payload.len(), _test.unwrap().len());

                let payload = match response_payload {
                    Ok(payload) => match zstd::encode_all(payload.as_slice(), 4) {
                        Ok(payload) => Some(payload),
                        Err(_) => {
                            eprintln!("[{}] Failed to compress response payload.", req.message_id);
                            return None;
                        }
                    },
                    Err(payload) => {
                        eprintln!("[{}] Failed to process request with command: {:?}", req.message_id, req.cmd);
                        eprintln!("[{}] - send error: {:?}", req.message_id, String::from_utf8_lossy(&payload));
                        return Some(Response {
                            creation_timestamp: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                            message_id: req.message_id,
                            status_code: StatusCode::IncorrectParams,
                            cmd: req.cmd,
                            payload: payload
                        })
                    }
                };

                match payload {
                    Some(response_payload) => {
                        return Some(Response {
                            creation_timestamp: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                            message_id: req.message_id,
                            status_code: StatusCode::Ok,
                            cmd: req.cmd,
                            payload: response_payload
                        })
                    },
                    None => {
                        eprintln!("[{}] Generated a None response payload: there was an error when processing the request response payload.", req.message_id);
                        return Some(Response {
                            creation_timestamp: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                            message_id: req.message_id,
                            status_code: StatusCode::InternalError,
                            cmd: req.cmd,
                            payload: vec![]
                        })
                    }
                }
            },
            RequestOrResponse::Response(res) => {
                eprintln!("[{}] Got a {} response from the server, but was expecting a request only.", res.message_id, res.cmd);
                return None;
            },
            RequestOrResponse::None => {
                /* A broken message, or waiting for the last chunk */
                return None;
            }
        }
    }
}