use std::{borrow::Cow, time::SystemTime};

use super::command_history::{CommandHistory};
use super::request_or_response::{RequestOrResponse, Response, StatusCode, ChunkedRequest, ChunkedResponse};

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
                    "ls" => match super::ls::process_ls_command(&req.payload) {
                        Some(payload) => Some(payload.to_string().as_bytes().to_vec()),
                        None => None
                    },
                    "download" => {
                        super::download::process_download_command(&req.payload)
                    },
                    _ => {
                        eprintln!("[{}] Got request with unknown command: {:?}", req.message_id, req.cmd);
                        None
                    }
                };

                match response_payload {
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
                        eprintln!("[{}] Generated an empty response payload. Maybe there was an error when processing the request.", req.message_id);
                        return None;
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