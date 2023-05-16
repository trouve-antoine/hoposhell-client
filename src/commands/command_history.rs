use std::{borrow::Cow, time::{SystemTime, UNIX_EPOCH}};

use super::request_or_response::{ChunkedRequestOrResponse, RequestOrResponse, Request, Response, ChunkType};

pub struct CommandHistory {
    past_requests: Vec<Request>,
    past_responses: Vec<Response>,
}

impl CommandHistory {
    pub fn new() -> CommandHistory {
        return CommandHistory {
            past_requests: vec![],
            past_responses: vec![]
        }
    }

    pub fn append(&mut self, msg: Cow<str>) -> RequestOrResponse {
        let req_or_res = ChunkedRequestOrResponse::deserialize(msg);

        match req_or_res {
            ChunkedRequestOrResponse::Request(req) => {
                let past_pos: Option<usize> = self.past_requests.iter().position(|past_request| {
                    past_request.message_id == req.message_id
                });
                match past_pos {
                    Some(pos) => {
                        let past_request = &mut self.past_requests[pos];
                        past_request.payload.push_str(req.payload.as_str());

                        match req.chunk_type {
                            ChunkType::Last => {
                                let past_request = self.past_requests.remove(pos);
                                return RequestOrResponse::Request(past_request);
                            },
                            _ => {
                                return RequestOrResponse::None;
                            }
                        }
                    },
                    None => {
                        let new_request = Request {
                            creation_timestamp: req.creation_timestamp,
                            cmd: req.cmd,
                            message_id: req.message_id,
                            target: req.target,
                            payload: req.payload
                        };
                        match req.chunk_type {
                            ChunkType::Last => {
                                return RequestOrResponse::Request(new_request);
                            },
                            _ => {
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
                        let past_response = &mut self.past_responses[pos];
                        past_response.payload.push_str(res.payload.as_str());

                        match res.chunk_type {
                            ChunkType::Last => {
                                let past_response = self.past_responses.remove(pos);
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
                        match res.chunk_type {
                            ChunkType::Last => {
                                return RequestOrResponse::Response(new_response);
                            },
                            _ => {
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