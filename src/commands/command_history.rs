use std::{time::{SystemTime, UNIX_EPOCH}};

use super::request_or_response::{ChunkedRequestOrResponse, RequestOrResponse, Request, Response, ChunkType};

pub struct CommandHistory {
    past_requests: Vec<Request>,
    past_responses: Vec<Response>,
    buffered_bytes: usize,
}

impl CommandHistory {
    pub fn new() -> CommandHistory {
        return CommandHistory {
            past_requests: vec![],
            past_responses: vec![],
            buffered_bytes: 0,
        }
    }

    pub fn append(&mut self, msg: &Vec<u8>) -> RequestOrResponse {
        let req_or_res = ChunkedRequestOrResponse::deserialize(msg);

        match req_or_res {
            ChunkedRequestOrResponse::Request(req) => {
                let past_pos: Option<usize> = self.past_requests.iter().position(|past_request| {
                    past_request.message_id == req.message_id
                });
                match past_pos {
                    Some(pos) => {
                        self.buffered_bytes = crate::buffer_limits::growth(
                            self.buffered_bytes, req.payload.len(), "pending commands");
                        let past_request = &mut self.past_requests[pos];
                        crate::buffer_limits::append(&mut past_request.payload, &req.payload, "command payload");

                        match req.chunk_type {
                            ChunkType::Last => {
                                let past_request = self.past_requests.remove(pos);
                                self.buffered_bytes -= request_size(&past_request);
                                return RequestOrResponse::Request(past_request);
                            },
                            _ => {
                                return RequestOrResponse::None;
                            }
                        }
                    },
                    None => {
                        let new_request = Request {
                            cmd: req.cmd,
                            message_id: req.message_id,
                            target: req.target,
                            payload: req.payload
                        };
                        let total = crate::buffer_limits::growth(
                            self.buffered_bytes, request_size(&new_request), "pending commands");
                        match req.chunk_type {
                            ChunkType::Last => {
                                return RequestOrResponse::Request(new_request);
                            },
                            _ => {
                                self.buffered_bytes = total;
                                self.past_requests.reserve_exact(1);
                                self.past_requests.push(new_request);
                                return RequestOrResponse::None;
                            }
                        }
                    }
                }
            },
            ChunkedRequestOrResponse::Response(res) => {
                let past_pos: Option<usize> = self.past_responses.iter().position(|past_request| {
                    past_request.message_id == res.message_id
                });
                match past_pos {
                    Some(pos) => {
                        self.buffered_bytes = crate::buffer_limits::growth(
                            self.buffered_bytes, res.payload.len(), "pending commands");
                        let past_response = &mut self.past_responses[pos];
                        crate::buffer_limits::append(&mut past_response.payload, &res.payload, "command payload");

                        match res.chunk_type {
                            ChunkType::Last => {
                                let past_response = self.past_responses.remove(pos);
                                self.buffered_bytes -= response_size(&past_response);
                                return RequestOrResponse::Response(past_response);
                            },
                            _ => {
                                return RequestOrResponse::None;
                            }
                        }
                    },
                    None => {
                        let new_response = Response {
                            creation_timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                            cmd: res.cmd,
                            message_id: res.message_id,
                            status_code: res.status_code,
                            payload: res.payload
                        };
                        let total = crate::buffer_limits::growth(
                            self.buffered_bytes, response_size(&new_response), "pending commands");
                        match res.chunk_type {
                            ChunkType::Last => {
                                return RequestOrResponse::Response(new_response);
                            },
                            _ => {
                                self.buffered_bytes = total;
                                self.past_responses.reserve_exact(1);
                                self.past_responses.push(new_response);
                                return RequestOrResponse::None;
                            }
                        }
                    }
                }
            },
            ChunkedRequestOrResponse::None => {
                return RequestOrResponse::None;
            }
        }

        
    }
}

fn request_size(request: &Request) -> usize {
    std::mem::size_of::<Request>() + request.cmd.len() + request.message_id.len()
        + request.target.len() + request.payload.len()
}

fn response_size(response: &Response) -> usize {
    std::mem::size_of::<Response>() + response.cmd.len() + response.message_id.len()
        + response.payload.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_commands_release_pending_budget() {
        let request = Request {
            cmd: "download".into(),
            message_id: "test".into(),
            target: "shell:test".into(),
            payload: vec![b'a'; crate::constants::COMMAND_PAYLOAD_SIZE + 10],
        };
        let mut history = CommandHistory::new();
        let mut chunks = request.chunk().into_iter();
        assert!(matches!(history.append(&chunks.next().unwrap().to_message_payload()), RequestOrResponse::None));
        assert!(history.buffered_bytes > 0);
        match history.append(&chunks.next().unwrap().to_message_payload()) {
            RequestOrResponse::Request(assembled) => assert_eq!(assembled.payload, request.payload),
            _ => panic!("missing assembled request"),
        }
        assert_eq!(history.buffered_bytes, 0);
        assert!(history.past_requests.is_empty());
    }

    #[test]
    fn completed_responses_release_pending_budget() {
        let response = Response {
            creation_timestamp: 0,
            cmd: "download".into(),
            message_id: "test".into(),
            status_code: super::super::request_or_response::StatusCode::Ok,
            payload: vec![b'a'; crate::constants::COMMAND_PAYLOAD_SIZE + 10],
        };
        let mut history = CommandHistory::new();
        let mut chunks = response.chunk().into_iter();
        assert!(matches!(history.append(&chunks.next().unwrap().to_message_payload()), RequestOrResponse::None));
        assert!(history.buffered_bytes > 0);
        assert!(matches!(history.append(&chunks.next().unwrap().to_message_payload()), RequestOrResponse::Response(_)));
        assert_eq!(history.buffered_bytes, 0);
        assert!(history.past_responses.is_empty());
    }
}
