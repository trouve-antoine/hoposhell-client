use std::{io::{Read, Write}, path::Path, net::TcpStream, thread};

use crate::{
    commands::{
        request_or_response::{
            Response, ChunkedResponse, ChunkType, StatusCode
        }
    },
    connect::{
        ReadMessageResult, read_messages_from_stream, send_message_to_stream, self, compute_hostname
    },
    ParseCommandResponseResult,
    message::{
        MessageTypeToStream, Message, MessageTypeToCmd
    },
    args::Args,
    make_random_id
};

use super::{download, ls, glob, request_or_response::{Request, ChunkedRequestOrResponse}};

pub fn main_command(args: Args) {
    let target_shell_id = &args.extra_args[0];
    let command = &args.extra_args[1];

    let current_shell_id = args.get_shell_id();
    if current_shell_id.is_none() {
        eprintln!("Please specify the shell id");
        std::process::exit(-1);
    }
    let current_shell_id = current_shell_id.unwrap();

    let make_id = || {  
        let random_str = make_random_id(8);
        return format!("{}:{}", current_shell_id, random_str)
    }; 

    let req:Option<Request>;
    let process_res: Box<dyn Fn(Response)>;

    match command.as_str() {
        ls::COMMAND_NAME => {
            // hopo command <shell_id> ls <folder_path>
            let folder_path = &args.extra_args[2];
            req = Some(ls::make_ls_request(make_id, &target_shell_id, &folder_path));
            process_res = Box::new(|res: Response| {
                ls::process_ls_response(&res.payload, args.format);
            });
        },
        download::COMMAND_NAME => {
            // hopo command <shell_id> download <remote_file_path> <local_file_path>
            let remote_file_path = &args.extra_args[2];
            req = Some(download::make_download_request(make_id, &target_shell_id, &remote_file_path));
            process_res = Box::new(|res: Response| {
                if args.extra_args.len() < 4 {
                    eprintln!("Please specify the local file path");
                    let local_path = String::from(Path::new("./").join(Path::new(&args.extra_args[2]).file_name().unwrap()).to_str().unwrap());
                    download::process_download_response(&res.payload, &local_path);
                } else {
                    download::process_download_response(&res.payload, &args.extra_args[3]);
                };
            });
        },
        glob::COMMAND_NAME => {
            // hopo command <shell_id> glob <pattern>
            let glob_pattern = &args.extra_args[2];
            req = Some(glob::make_glob_request(make_id, &target_shell_id, &glob_pattern));
            process_res = Box::new(|res: Response| {
                glob::process_glob_response(&res.payload, args.format);
            });
        },
        _ => {
            eprintln!("Command {} is unknown", command);
            std::process::exit(-1);
        }
    };

    let req = req.unwrap();

    if let None = args.server_crt_path {
        eprintln!("Please specify env var HOPOSHELL_SERVER_CRT, or run `hopo setup` to download it to the default location.");
        std::process::exit(-1);
    }

    if let None = args.shell_key_path {
        eprintln!("Please specify env var HOPOSHELL_SHELL_KEY, or specify the shell_id parameter of the connect command.");
        std::process::exit(-1);
    }

    let ssl_connector = if args.use_ssl {
        Some(connect::make_ssl_conector(
            args.server_crt_path.as_ref().unwrap(),
            args.shell_key_path.as_ref().unwrap(),
            args.verify_crt
        ))
    } else {
        None
    };

    let tcp_stream = TcpStream::connect(args.server_url.as_str());
    if let Err(e) = tcp_stream {
        eprintln!("Unable to connect to hoposhell server: {}", e);
        std::process::exit(-1);
    }
    let tcp_stream = tcp_stream.unwrap();

    if let Some(ref ssl_connector) = ssl_connector {
        let hostname = compute_hostname(&args.server_url);
        let ssl_stream = ssl_connector.connect(hostname, tcp_stream).unwrap();
        handle_command_connection(&args, ssl_stream, &req, &process_res)
    } else {
        handle_command_connection(&args, tcp_stream, &req, &process_res)
    };

    
}

fn handle_command_connection(
    args: &Args,
    mut stream: impl Read + Write,
    req: &Request,
    process_res: &impl Fn(Response)
) {
    let header_message = Message {
        mtype: MessageTypeToStream::HEADER,
        content: Some(format!("v{}/command", args.version).as_bytes().to_vec())
    };
    match send_message_to_stream(&header_message, &mut stream) {
        Ok(_) => {},
        Err(e) => {
            eprintln!("[{}] Unable to send header message: {}", req.message_id, e);
            std::process::exit(-1);
        }
    }

    let chunks = req.chunk();
    eprintln!("[{}] Send request {} with #chunks: {}", req.message_id, req.cmd, chunks.len());
    for chunk in chunks {
        // eprintln!("- send: {} {} {:?}", chunk.cmd, chunk.message_id, chunk.chunk_type);
        let msg_payload = chunk.to_message_payload();
        let msg = Message {
            mtype: MessageTypeToStream::COMMAND,
            content: Some(msg_payload)
        };
        match send_message_to_stream(&msg, &mut stream) {
            Ok(_) => {},
            Err(e) => {
                eprintln!("[{}] Unable to send command message: {}", req.message_id, e);
                std::process::exit(-1);
            }
        }
    }

    let mut buf_str = String::from("");
    let mut all_res: Vec<ChunkedResponse> = vec![];
    let start_time = std::time::Instant::now();

    loop {
        if start_time.elapsed() > args.command_timeout {
            eprintln!("[{}] Command timeout", req.message_id);
            std::process::exit(-1);
        }

        match read_messages_from_stream(&mut stream, &mut buf_str) {
            ReadMessageResult::Ok(messages) => {
                match parse_command_response_message(&req, &messages, &mut all_res) {
                    ParseCommandResponseResult::CanContinue => {
                        /* NOP */
                    },
                    ParseCommandResponseResult::ReachedLastChunk => {
                        break;
                    },
                    ParseCommandResponseResult::Error => {
                        std::process::exit(-1);
                    }
                }
            },
            ReadMessageResult::CanContinue => {
                if args.read_timeout_sleep.as_millis() > 0 {
                    eprint!("*");
                    thread::sleep(args.read_timeout_sleep);
                }
            },
            ReadMessageResult::CannotContinue => {
                eprint!("[{}] Got an error when reading the tcp stream.", req.message_id);
                std::process::exit(-1);
            }
        }
    }

    if all_res.len() == 0 {
        eprintln!("[{}] Got no response", req.message_id);
        std::process::exit(-1);
    };

    
    let res = Response {
        creation_timestamp: all_res[0].creation_timestamp,
        cmd: all_res[0].cmd.clone(),
        message_id: all_res[0].message_id.clone(),
        status_code: all_res[0].status_code,
        payload: all_res.iter().map(|res| res.payload.clone()).into_iter().flatten().collect()
    };

    eprintln!("[{}] Total number of response chunk: {}", res.message_id, all_res.len());

    match zstd::decode_all(res.payload.as_slice()) {
        Ok(decompressed) => {
            // glob::process_glob_response(decompressed.as_slice());
            process_res(Response {
                creation_timestamp: res.creation_timestamp,
                cmd: res.cmd,
                message_id: res.message_id,
                status_code: res.status_code,
                payload: decompressed
            });
        },
        Err(e) => {
            eprintln!("Unable to decompress glob response: {}", e);
            std::process::exit(-1);
        }
    }
}

fn parse_command_response_message(
    req: &Request,
    messages: &Vec<Message<MessageTypeToCmd>>,
    all_res: &mut Vec<ChunkedResponse>
) -> ParseCommandResponseResult {
    
    for message in messages.iter() {
        if message.mtype != MessageTypeToCmd::COMMAND {
            eprintln!("Unexpected message type: {:?}", message.mtype);
            return ParseCommandResponseResult::Error;
        }

        // TODO: avoid clone
        let content: Vec<u8> = message.content.clone().unwrap();

        match ChunkedRequestOrResponse::deserialize(&content) {
            ChunkedRequestOrResponse::Request(_req_or_res) => {
                eprint!("Got a request, but was waiting for a resposnse: ignore...");
                return ParseCommandResponseResult::CanContinue;
            },
            ChunkedRequestOrResponse::None => {
                eprintln!("Got an empty command message: ignore");
            return ParseCommandResponseResult::CanContinue;
            },
            ChunkedRequestOrResponse::Response(res) => {
                eprintln!("[{}] Got a response: {} (chunk type: {:?})", res.message_id, res.cmd, res.chunk_type);
                if res.message_id != req.message_id {
                    eprintln!("[{}] Got a response with a unexpected message_id: {}. Ignore...", req.message_id, res.message_id);
                    return ParseCommandResponseResult::CanContinue;
                }
                if res.status_code != StatusCode::Ok {
                    eprintln!("[{}] Got a response with status {:?}: exit", res.message_id, res.status_code);
                    return ParseCommandResponseResult::Error;
                }
                let chunk_type = res.chunk_type.clone();
                all_res.push(res);
                if chunk_type == ChunkType::Last {
                    return ParseCommandResponseResult::ReachedLastChunk;
                }
            }
        }
    }
    return ParseCommandResponseResult::CanContinue;
}