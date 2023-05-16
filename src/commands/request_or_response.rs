use std::{borrow::Cow, time::SystemTime};

use crate::constants::BUF_SIZE;

enum ReqOrRes {
    Req,
    Res
}

impl ReqOrRes {
    pub fn maybe_from(v: Option<&[u8]>) -> Option<Self> {
        match v {
            Some(v) => match std::str::from_utf8(v) {
                Ok(v) => match v {
                    "req" => Some(ReqOrRes::Req),
                    "res" => Some(ReqOrRes::Res),
                    _ => None
                },
                Err(_) => None
            }
            None => None
        }
    }
}

pub enum StatusCode {
    Ok,
    IncorrectParams
}

impl StatusCode {
    pub fn maybe_from(code: Option<String>) -> Option<Self> {
        match code {
            None => None,
            Some(code) => match code.parse::<i32>() {
                Ok(code) => match code {
                    200 => Some(StatusCode::Ok),
                    400 => Some(StatusCode::IncorrectParams),
                    _ => None
                },
                Err(_) => None
            }
        }
    }
}

pub enum ChunkType {
    NotLast,
    Last
}

impl ChunkType {
    pub fn maybe_from(v: Option<&[u8]>) -> Option<Self> {
        match v {
            Some(v) => match std::str::from_utf8(v) {
                Ok(v) => match v {
                    "not-last" => Some(ChunkType::NotLast),
                    "last" => Some(ChunkType::Last),
                    _ => None
                },
                Err(_) => None
            }
            None => None
        }
    }
}

pub struct ChunkedRequest {
    pub creation_timestamp: u64,
    pub cmd: String,
    pub message_id: String,
    pub target: String,
    pub chunk_type: ChunkType,
    pub payload: Vec<u8>
}

pub struct Request {
    pub creation_timestamp: u64,
    pub cmd: String,
    pub message_id: String,
    pub target: String,
    pub payload: Vec<u8>
}

impl Request {
    pub fn chunk(self) -> Vec<ChunkedRequest> {
        // Homework:
        // - return an iterator instead
        // - use a slice for the payload to avoid copying data

        // chunk self.payload into chunks
        let mut all_chunked_requests: Vec<ChunkedRequest> = vec![];

        for chunk in self.payload.chunks(BUF_SIZE) {
            all_chunked_requests.push(ChunkedRequest {
                creation_timestamp: self.creation_timestamp,
                cmd: self.cmd.clone(),
                message_id: self.message_id.clone(),
                target: self.target.clone(),
                chunk_type: ChunkType::NotLast,
                payload: chunk.to_vec()
            });
        }

        all_chunked_requests.last().unwrap().chunk_type = ChunkType::Last;

        return all_chunked_requests;
    }
}

pub struct ChunkedResponse {
    pub creation_timestamp: u64,
    pub cmd: String,
    pub message_id: String,
    pub status_code: StatusCode,
    pub chunk_type: ChunkType,
    pub payload: Vec<u8>
}

pub struct Response {
    pub creation_timestamp: u64,
    pub cmd: String,
    pub message_id: String,
    pub status_code: StatusCode,
    pub payload: Vec<u8>
}

impl Response {
    pub fn chunk(self) -> Vec<ChunkedResponse> {
        // Homework:
        // - return an iterator instead
        // - use a slice for the payload to avoid copying data

        // chunk self.payload into chunks
        let mut all_chunked_responses: Vec<ChunkedResponse> = vec![];

        for chunk in self.payload.chunks(BUF_SIZE) {
            all_chunked_responses.push(ChunkedResponse {
                creation_timestamp: self.creation_timestamp,
                cmd: self.cmd.clone(),
                message_id: self.message_id.clone(),
                status_code: self.status_code,
                chunk_type: ChunkType::NotLast,
                payload: chunk.to_vec()
            });
        }

        all_chunked_responses.last().unwrap().chunk_type = ChunkType::Last;

        return all_chunked_responses;
    }
}

pub enum RequestOrResponse {
    Request(Request),
    Response(Response),
    None
}

pub enum ChunkedRequestOrResponse {
    Request(ChunkedRequest),
    Response(ChunkedResponse),
    None
}

impl ChunkedRequestOrResponse {
    pub fn deserialize(msg: &[u8]) -> Self {
        // Request: cmd/req/42/shell:42/last/~/.ssh
        // Response: cmd/res/42/200/last/{ file1, file2 }

        let mut parts =  msg.splitn(6, |x| x == &crate::constants::MESSAGE_PARTS_SEPARATOR);

        let cmd = maybe_string(parts.next());
        let req_or_res = ReqOrRes::maybe_from(parts.next());
        let message_id = maybe_string(parts.next());
        let target_or_status = maybe_string(parts.next());
        let chunk_type = ChunkType::maybe_from(parts.next());
        let payload = parts.next();

        if cmd.is_none() {
            eprintln!("Got command with unknown cmd: {:?}", msg);
            return ChunkedRequestOrResponse::None;
        }

        if req_or_res.is_none() {
            eprintln!("Got command with unknown req_or_res: {:?}", msg);
            return ChunkedRequestOrResponse::None;
        }

        if message_id.is_none() {
            eprintln!("Got command with unknown message_id: {:?}", msg);
            return ChunkedRequestOrResponse::None;
        }
        
        if target_or_status.is_none() {
            eprintln!("Got command with unknown target_or_status: {:?}", msg);
            return ChunkedRequestOrResponse::None;
        }

        if chunk_type.is_none() {
            eprintln!("Got command with unknown chunk_type: {:?}", msg);
            return ChunkedRequestOrResponse::None;
        }

        if payload.is_none() {
            eprintln!("Got command with unknown payload: {:?}", msg);
            return ChunkedRequestOrResponse::None;
        }

        match req_or_res.unwrap() {
            ReqOrRes::Req => {
                ChunkedRequestOrResponse::Request(ChunkedRequest {
                    creation_timestamp: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                    cmd: cmd.unwrap(),
                    message_id: message_id.unwrap(),
                    target: target_or_status.unwrap(),
                    chunk_type: chunk_type.unwrap(),
                    payload: payload.unwrap().to_vec()
                })
            },
            ReqOrRes::Res => {
                let status_code = StatusCode::maybe_from(target_or_status);
                if status_code.is_none() {
                    eprintln!("Got command with unknown status_code: {:?}", msg);
                    return ChunkedRequestOrResponse::None;
                }

                ChunkedRequestOrResponse::Response(ChunkedResponse {
                    creation_timestamp: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                    cmd: cmd.unwrap().to_string(),
                    message_id: message_id.unwrap().to_string(),
                    status_code: status_code.unwrap(),
                    chunk_type: chunk_type.unwrap(),
                    payload: payload.unwrap().to_vec()
                })
            }
        }
    }
}

pub fn maybe_string(v: Option<&[u8]>) -> Option<String> {
    match v {
        Some(v) => match String::from_utf8(v.to_vec()) {
            Ok(v) => Some(v),
            Err(_) => None
        },
        None => None
    }
}