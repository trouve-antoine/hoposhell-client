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

    pub fn process_msg(&mut self, msg: Cow<str>) -> Option<Response> {
        /* Parses and processes a command message in serialized form */
        /* (parsing is actually done inside command_history) */
        let cmd = self.history.append(msg);

        match cmd {
            RequestOrResponse::Request(req) => {
                /* Got a request from the cloud or another shell */
                /* This happens in the loop that processes incomming messages from the server */

                let response_payload = match req.cmd.as_str() {
                    "ls" => {
                        super::ls::process_ls_command(&req.payload)
                    },
                    _ => {
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
                            payload: response_payload.to_string().as_bytes().to_vec()
                        })
                    },
                    None => {
                        return None;
                    }
                }
            },
            RequestOrResponse::Response(res) => {
                /* Got a response from a request we made to the cloud or another shell */
                /* This happens inside a call to "hopo command" */

                match res.cmd.as_str() {
                    "ls" => {
                        super::ls::process_ls_response(&res.payload);
                    },
                    cmd => {
                        eprintln!("Got response with unknown command: {:?}", cmd);
                    }
                }

                return None;
            },
            RequestOrResponse::None => {
                /* A broken message, or waiting for the last chunk */
                return None;
            }
        }
    }
}