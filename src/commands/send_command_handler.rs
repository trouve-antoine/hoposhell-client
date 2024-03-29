use std::{io::{Read, Write}, net::TcpStream, thread};

use openssl::ssl::SslConnector;
use serde_json::{Value};

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

use super::{download, tcp, ls, http, glob, scripts, request_or_response::{Request, ChunkedRequestOrResponse}};

pub fn main_command(args: Args) {
    let target_shell_id = &args.extra_args[0];
    let command = &args.extra_args[1];

    let command_args = &args.extra_args[2..].to_vec();

    return send_command(target_shell_id, command, command_args, &args);
}

pub fn send_command(
    target_shell_id: &String,
    command: &String,
    command_args: &Vec<String>,
    args: &Args
) {
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
            let folder_path = &command_args[0];
            req = Some(ls::make_ls_request(make_id, &target_shell_id, &folder_path));
            process_res = Box::new(|res: Response| {
                ls::process_ls_response(&res.payload, args.format);
            });
        },
        download::COMMAND_NAME | download::COMMAND_ALIAS => {
            // hopo command <shell_id> download <remote_file_path> <local_file_path>
            let remote_file_path = &command_args[0];
            let local_file_path = if command_args.len() < 2 { None } else {
                Some(String::from(&command_args[1]))
            };

            req = Some(download::make_download_request(make_id, &target_shell_id, &remote_file_path));

            process_res = Box::new(move |res: Response| {
                download::process_download_response(&res.payload, remote_file_path, local_file_path.clone());
            });
        },
        glob::COMMAND_NAME => {
            // hopo command <shell_id> glob <pattern>
            let glob_pattern = &command_args[0];
            req = Some(glob::make_glob_request(make_id, &target_shell_id, &glob_pattern));
            process_res = Box::new(|res: Response| {
                glob::process_glob_response(&res.payload, args.format);
            });
        },
        http::COMMAND_NAME => {
            // hopo command <shell_id> http <verb> <url>
            req = Some(http::make_http_request(make_id, &target_shell_id, &command_args));
            process_res = Box::new(|res: Response| {
                http::process_http_response(&res.payload, args.format);
            });
        },
        tcp::COMMAND_NAME => {
            // hopo command <shell_id> tcp host port payload
            let host = &command_args[0];
            let port = &command_args[1];
            let payload = &command_args[2];

            let port: u16 = port.parse().unwrap();
            
            req = Some(tcp::make_tcp_request(make_id, &target_shell_id, host.clone(), port, payload.clone().as_bytes().to_vec()));
            process_res = Box::new(|res: Response| {
                tcp::process_tcp_response(&res.payload, args.format);
            });
        },
        scripts::COMMAND_NAME => {
            // hopo command <shell_id> tcp host port payload
            let script_name = &command_args[0];
            
            req = Some(scripts::make_scripts_request(make_id, &target_shell_id, script_name.clone()));
            process_res = Box::new(|res: Response| {
                scripts::process_script_response(&res.payload, args.format);
            });
        },
        _ => {
            eprintln!("Command {} is unknown", command);
            std::process::exit(-1);
        }
    };

    let req = req.unwrap();

    let (ssl_connector, tcp_stream) = connect_to_hoposhell(args);

    if let Some(ref ssl_connector) = ssl_connector {
        let hostname = compute_hostname(&args.server_url);
        let ssl_stream = ssl_connector.connect(hostname, tcp_stream).unwrap();
        handle_command_connection(&args, ssl_stream, &req, &process_res, args.verbose)
    } else {
        handle_command_connection(&args, tcp_stream, &req, &process_res, args.verbose)
    };

    
}

pub fn connect_to_hoposhell(args: &Args) -> (Option<SslConnector>, TcpStream) {
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

    return (ssl_connector, tcp_stream);
}

fn handle_command_connection(
    args: &Args,
    mut stream: impl Read + Write,
    req: &Request,
    process_res: &impl Fn(Response),
    verbose: bool
) {
    let res = send_request_and_get_response(args, &mut stream, req, verbose);

    match res {
        Ok(res) => {
            process_res(res);
        },
        Err(e) => {
            eprintln!("[{}] Unable to send request: {}", req.message_id, e);
            std::process::exit(-1);
        }
    }
}

pub fn send_request_and_get_response(
    args: &Args,
    mut stream: impl Read + Write,
    req: &Request,
    verbose: bool
) -> Result<Response, std::io::Error> {
    let header_message = Message {
        mtype: MessageTypeToStream::HEADER,
        content: Some(format!("v{}/command", args.version).as_bytes().to_vec())
    };
    match send_message_to_stream(&header_message, &mut stream, verbose) {
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
        match send_message_to_stream(&msg, &mut stream, verbose) {
            Ok(_) => {},
            Err(e) => {
                eprintln!("[{}] Unable to send command message: {}", req.message_id, e);
            }
        }
    }

    let mut buf_str = String::from("");
    let mut all_res: Vec<ChunkedResponse> = vec![];
    let mut start_time = std::time::Instant::now();

    eprint!("Recieved: 0 bytes\r");
    let mut total_bytes_received = 0;

    loop {
        if start_time.elapsed() > args.command_timeout {
            eprintln!("[{}] Command timeout", req.message_id);
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Command timeout"));
        }

        match read_messages_from_stream(&mut stream, &mut buf_str, verbose) {
            ReadMessageResult::Ok(messages) => {
                start_time = std::time::Instant::now();
                match parse_command_response_message(&req, &messages, &mut all_res) {
                    ParseCommandResponseResult::CanContinue => {
                        total_bytes_received += messages.iter().fold(0, |acc, msg| acc + msg.content.as_ref().unwrap().len());
                        eprint!("Recieved: {} bytes\r", total_bytes_received);
                    },
                    ParseCommandResponseResult::ReachedLastChunk => {
                        break;
                    },
                    ParseCommandResponseResult::Error => {
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Unable to parse command response"));
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
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "Unable to read tcp stream"));
            }
        }
    }

    if all_res.len() == 0 {
        eprintln!("[{}] Got no response", req.message_id);
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Got no response"));
    };

    let message_id = all_res[0].message_id.clone();
    let compressed_payload: Vec<u8> = all_res.iter().map(|res| res.payload.clone()).into_iter().flatten().collect();

    eprintln!("[{}] Total number of response chunk: {}", message_id, all_res.len());

    let payload = zstd::decode_all(compressed_payload.as_slice());
    if let Err(e) = payload {
        eprintln!("Unable to decompress response: {}", e);
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Unable to decompress response"));
    }
    let payload = payload.unwrap();

    return Ok(Response {
        creation_timestamp: all_res[0].creation_timestamp,
        cmd: all_res[0].cmd.clone(),
        message_id,
        status_code: all_res[0].status_code,
        payload
    });
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
                // eprintln!("[{}] Got a response: {} (chunk type: {:?})", res.message_id, res.cmd, res.chunk_type);
                if res.message_id != req.message_id {
                    eprintln!("[{}] Got a response with a unexpected message_id: {}. Ignore...", req.message_id, res.message_id);
                    return ParseCommandResponseResult::CanContinue;
                }
                if res.status_code != StatusCode::Ok {
                    eprintln!("[{}] Got a response with status {:?}: exit", res.message_id, res.status_code);
                    let error_body = std::str::from_utf8(res.payload.as_slice());

                    if let Ok(error_body) = error_body {
                        let error_json: Result<Value, _> = serde_json::from_str(error_body);
                        if let Ok(error_json) = error_json {
                            if let Some(error) = error_json.get("error") {
                                eprintln!("[{}] {}", res.message_id, error.as_str().unwrap());
                                return ParseCommandResponseResult::Error;
                            }
                        } else {
                            // eprintln!("[{}] Error body was: {}", res.message_id, error_body);
                            eprintln!("[{}] {}", res.message_id, error_body);
                        }
                    }
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