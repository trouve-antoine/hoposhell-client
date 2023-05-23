use std::{time::SystemTime};

use crate::constants::{COMMAND_PAYLOAD_SIZE};

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
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            ReqOrRes::Req => b"req".to_vec(),
            ReqOrRes::Res => b"res".to_vec()
        }
    }
}
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum StatusCode {
    Ok,
    IncorrectParams,
    InternalError,
}

impl StatusCode {
    pub fn maybe_from(code: Option<String>) -> Option<Self> {
        match code {
            None => None,
            Some(code) => match code.parse::<i32>() {
                Ok(code) => match code {
                    200 => Some(StatusCode::Ok),
                    400 => Some(StatusCode::IncorrectParams),
                    500 => Some(StatusCode::InternalError),
                    _ => None
                },
                Err(_) => None
            }
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            StatusCode::Ok => b"200".to_vec(),
            StatusCode::IncorrectParams => b"400".to_vec(),
            StatusCode::InternalError => b"500".to_vec()
        }
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
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

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            ChunkType::NotLast => b"not-last".to_vec(),
            ChunkType::Last => b"last".to_vec()
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

impl ChunkedRequest {
    pub fn to_message_payload(mut self) -> Vec<u8> {
        let mut payload = vec![];

        payload.append(&mut self.cmd.as_bytes().to_vec());
        payload.push(crate::constants::MESSAGE_PARTS_SEPARATOR);
        payload.append(&mut ReqOrRes::Req.to_bytes());
        payload.push(crate::constants::MESSAGE_PARTS_SEPARATOR);
        payload.append(&mut self.message_id.as_bytes().to_vec());
        payload.push(crate::constants::MESSAGE_PARTS_SEPARATOR);
        payload.append(&mut self.target.as_bytes().to_vec());
        payload.push(crate::constants::MESSAGE_PARTS_SEPARATOR);
        payload.append(&mut self.chunk_type.to_bytes());
        payload.push(crate::constants::MESSAGE_PARTS_SEPARATOR);
        payload.append(&mut self.payload);

        return payload;
    }
}

#[derive(Debug)]
pub struct Request {
    pub cmd: String,
    pub message_id: String,
    pub target: String,
    pub payload: Vec<u8>
}

impl Request {
    pub fn chunk(&self) -> Vec<ChunkedRequest> {
        // Homework:
        // - return an iterator instead
        // - use a slice for the payload to avoid copying data

        // chunk self.payload into chunks
        let mut all_chunked_requests: Vec<ChunkedRequest> = vec![];

        for chunk in self.payload.chunks(COMMAND_PAYLOAD_SIZE) {
            all_chunked_requests.push(ChunkedRequest {
                creation_timestamp: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                cmd: self.cmd.clone(),
                message_id: self.message_id.clone(),
                target: self.target.clone(),
                chunk_type: ChunkType::NotLast,
                payload: chunk.to_vec()
            });
        }

        let last_req = all_chunked_requests.last_mut();
        
        if let Some(last_req) = last_req {
            last_req.chunk_type = ChunkType::Last
        }

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

impl ChunkedResponse {
    pub fn to_message_payload(mut self) -> Vec<u8> {
        let mut payload = vec![];

        payload.append(&mut self.cmd.as_bytes().to_vec());
        payload.push(crate::constants::MESSAGE_PARTS_SEPARATOR);
        payload.append(&mut ReqOrRes::Res.to_bytes());
        payload.push(crate::constants::MESSAGE_PARTS_SEPARATOR);
        payload.append(&mut self.message_id.as_bytes().to_vec());
        payload.push(crate::constants::MESSAGE_PARTS_SEPARATOR);
        payload.append(&mut self.status_code.to_bytes());
        payload.push(crate::constants::MESSAGE_PARTS_SEPARATOR);
        payload.append(&mut self.chunk_type.to_bytes());
        payload.push(crate::constants::MESSAGE_PARTS_SEPARATOR);
        payload.append(&mut self.payload);

        return payload;
    }
}

#[derive(Debug)]
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

        let payload_chunks = match self.payload.len() {
            0 => vec!["".as_bytes()],
            _ => self.payload.chunks(COMMAND_PAYLOAD_SIZE).collect::<Vec<&[u8]>>()
        };

        eprintln!("[{}] #chunks: {:?}", self.message_id, payload_chunks.len());

        for chunk in payload_chunks {
            all_chunked_responses.push(ChunkedResponse {
                creation_timestamp: self.creation_timestamp,
                cmd: self.cmd.clone(),
                message_id: self.message_id.clone(),
                status_code: self.status_code,
                chunk_type: ChunkType::NotLast,
                payload: chunk.to_vec()
            });
        }

        let last_res = all_chunked_responses.last_mut();

        if let Some(last_res) = last_res {
            last_res.chunk_type = ChunkType::Last
        }

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
    pub fn deserialize(msg: &Vec<u8>) -> Self {
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_chunked_request_single_chunk() {
        let req = super::Request {
            cmd: "cmd".to_string(),
            message_id: "42".to_string(),
            target: "shell:42".to_string(),
            payload: "~/.ssh".as_bytes().to_vec()
        };

        let chunked_reqs = req.chunk();

        assert_eq!(chunked_reqs.len(), 1);
        let first_chunk = &chunked_reqs[0];

        assert_ne!(first_chunk.creation_timestamp, 0);
        assert_eq!(first_chunk.cmd, "cmd");
        assert_eq!(first_chunk.message_id, "42");
        assert_eq!(first_chunk.target, "shell:42");
        assert_eq!(first_chunk.chunk_type, super::ChunkType::Last);
        assert_eq!(first_chunk.payload, "~/.ssh".as_bytes().to_vec());
    }
    
    #[test]
    fn test_chunked_request_two_chunks() {
        let req = super::Request {
            cmd: "cmd".to_string(),
            message_id: "42".to_string(),
            target: "shell:42".to_string(),
            payload: [0; crate::constants::COMMAND_PAYLOAD_SIZE+200].to_vec()
        };

        let chunked_reqs = req.chunk();

        assert_eq!(chunked_reqs.len(), 2);
        let first_chunk = &chunked_reqs[0];
        let second_chunk = &chunked_reqs[1];

        assert_eq!(first_chunk.creation_timestamp, second_chunk.creation_timestamp);
        assert_eq!(first_chunk.cmd, second_chunk.cmd);
        assert_eq!(first_chunk.message_id, second_chunk.message_id);
        assert_eq!(first_chunk.target, second_chunk.target);
        assert_eq!(first_chunk.chunk_type, super::ChunkType::NotLast);
        assert_eq!(first_chunk.payload.len(), crate::constants::COMMAND_PAYLOAD_SIZE);
        assert_eq!(second_chunk.payload.len(), 200);
    }
    #[test]
    fn test_serialize_and_deserialize_request() {
        let req = super::Request {
            cmd: "cmd".to_string(),
            message_id: "42".to_string(),
            target: "shell:42".to_string(),
            payload: "~/.ssh".as_bytes().to_vec()
        };

        let chunks = req.chunk();
        assert_eq!(chunks.len(), 1);
        
        for chunk in chunks {
            let serialized = chunk.to_message_payload();
            let deserialized = super::ChunkedRequestOrResponse::deserialize(&serialized);

            match deserialized {
                super::ChunkedRequestOrResponse::Request(deserialized) => {
                    assert_eq!(deserialized.cmd, "cmd");
                    assert_eq!(deserialized.message_id, "42");
                    assert_eq!(deserialized.target, "shell:42");
                    assert_eq!(deserialized.chunk_type, super::ChunkType::Last);
                    assert_eq!(deserialized.payload, "~/.ssh".as_bytes().to_vec());
                },
                _ => assert!(false)
            }
        }
    }
    #[test]
    fn test_serialize_and_deserialize_response() {
        let res = super::Response {
            creation_timestamp: 0,
            cmd: "cmd".to_string(),
            message_id: "42".to_string(),
            status_code: super::StatusCode::Ok,
            payload: "~/.ssh".as_bytes().to_vec()
        };

        let chunks = res.chunk();
        assert_eq!(chunks.len(), 1);
        
        for chunk in chunks {
            let serialized = chunk.to_message_payload();
            let deserialized = super::ChunkedRequestOrResponse::deserialize(&serialized);

            match deserialized {
                super::ChunkedRequestOrResponse::Response(deserialized) => {
                    assert_eq!(deserialized.cmd, "cmd");
                    assert_eq!(deserialized.message_id, "42");
                    assert_eq!(deserialized.status_code, super::StatusCode::Ok);
                    assert_eq!(deserialized.chunk_type, super::ChunkType::Last);
                    assert_eq!(deserialized.payload, "~/.ssh".as_bytes().to_vec());
                },
                _ => assert!(false)
            }
        }
    }
}

pub fn make_shell_target(shell_id: &String) -> String {
    return format!("shell:{}", shell_id)
}