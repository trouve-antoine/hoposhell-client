use std::io::{self, Read, Write};

mod args;
mod message;
mod run_shell_command;
mod connect;
mod constants;
mod commands {
    pub mod request_or_response;
    pub mod command_processor;
    pub mod command_history;
    pub mod restart;
    pub mod resize;
    pub mod ls;
}

use commands::{ls::{make_ls_request, process_ls_command}, request_or_response::{ChunkType, Response, ChunkedResponse, Request}};
use connect::{send_message_to_stream, read_messages_from_stream, compute_hostname};
use rand::Rng;
use message::{MessageTypeToStream, Message};
use rand::{self, distributions::Alphanumeric};

use std::{
    path::Path,
    collections::HashMap, net::TcpStream, thread
};

use args::{Args, ArgsCommand};

use crate::{connect::ReadMessageResult, message::MessageTypeToCmd, commands::request_or_response::{ChunkedRequestOrResponse, StatusCode}};

fn main() {
    let args = args::parse_args();

    println!("Got command {:?}", args.command);

    if args.already_connected && args.command == ArgsCommand::CONNECT {
        eprintln!("Got command connect but the shell is already connected");
        std::process::exit(-1);
    }

    match args.command {
        ArgsCommand::CONNECT => connect::main_connect(args),
        ArgsCommand::SETUP => main_setup(args),
        ArgsCommand::DOWNLOAD => main_download(args),
        ArgsCommand::UPLOAD => main_upload(args),
        ArgsCommand::VERSION => {
            eprintln!("Hoposhell Client v{}", args.version)
        },
        ArgsCommand::COMMAND => {
            main_command(args);
        }
    }
}

fn main_setup(args: Args) {
    /* */
    match args.shell_name {
        Some(shell_name) => {
            println!("Get credentials for shell {}", shell_name);
            get_shell_credentials(
                shell_name, args.api_url, 
                args.server_crt_path.unwrap(),
                args.shell_key_path.unwrap(),
                args.hoposhell_folder_path
            );
        },
        None => {
            eprintln!("Please specify the shell name");
        }
    }
}

fn main_download(args: Args) {
    /* */
    let shell_id_and_path = split_shell_id_and_path(&args.extra_args[0]);
    if shell_id_and_path.is_none() {
        eprintln!("Please specify the file to download");
        std::process::exit(-1);
    }
    let shell_id_and_path = shell_id_and_path.unwrap();

    eprintln!("Download not implemented yet");
    std::process::exit(0);
}

fn main_upload(args: Args) {
    /* */
    let local_file = Path::new(&args.extra_args[0]);
    /* */
    let shell_id_and_path = split_shell_id_and_path(&args.extra_args[1]);
    if shell_id_and_path.is_none() {
        eprintln!("Please specify the upload destination");
        std::process::exit(-1);
    }
    let shell_id_and_path = shell_id_and_path.unwrap();

    eprintln!("Upload not implemented yet");
    std::process::exit(0);
}

fn main_command(args: Args) {
    let target_shell_id = &args.extra_args[0];
    let command = &args.extra_args[1];

    let current_shell_id = args.get_shell_id();
    if current_shell_id.is_none() {
        eprintln!("Please specify the shell id");
        std::process::exit(-1);
    }
    let current_shell_id = current_shell_id.unwrap();

    let make_id = || {  
        let random_str = make_random_id(4);
        return format!("{}:{}", current_shell_id, random_str)
    }; 

    let mut req:Option<Request> = None;
    let mut process_res: Option<fn(Response) -> ()> = None;

    match command.as_str() {
        "ls" => {
            // hopo command <shell_id> ls <folder_path>
            let folder_path = &args.extra_args[2];
            req = Some(make_ls_request(make_id, &target_shell_id, &folder_path));
            process_res = Some(|res: Response| {
                process_ls_command(res.payload.as_slice());
            } );
        },
        _ => {
            eprintln!("Command {} is unknown", command);
            std::process::exit(-1);
        }
    };

    let req = req.unwrap();
    let process_res = process_res.unwrap();

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
        handle_command_connection(&args, ssl_stream, &req, process_res)
    } else {
        handle_command_connection(&args, tcp_stream, &req, process_res)
    };

    
}

fn handle_command_connection(
    args: &Args,
    mut stream: impl Read + Write,
    req: &Request,
    process_res: fn(Response) -> ()
) {
    for chunk in req.chunk() {
        let msg_payload = chunk.to_message_payload();
        let msg = Message {
            mtype: MessageTypeToStream::COMMAND,
            content: Some(msg_payload)
        };
        send_message_to_stream(&msg, &mut stream);
    }

    let mut buf_str = String::from("");
    let mut all_res: Vec<ChunkedResponse> = vec![];
    let start_time = std::time::Instant::now();

    loop {
        if start_time.elapsed() > args.command_timeout {
            eprintln!("Command timeout");
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
                eprint!("Got an error when reading the tcp stream.");
                std::process::exit(-1);
            }
        }
    }

    if all_res.len() == 0 {
        eprintln!("Got no response");
        std::process::exit(-1);
    };

    let res = Response {
        creation_timestamp: all_res[0].creation_timestamp,
        cmd: all_res[0].cmd.clone(),
        message_id: all_res[0].message_id.clone(),
        status_code: all_res[0].status_code,
        payload: all_res.iter().map(|res| res.payload.clone()).into_iter().flatten().collect()
    };

    process_res(res);
}

fn get_shell_credentials(shell_name: String, api_url: String, server_crt_path: String, shell_key_path: String, _hoposhell_folder_path: String) {
    eprintln!("🪙 {}/shell-credentials/request/{}", api_url, shell_name);
    reqwest::blocking::get(format!("{}/shell-credentials/request/{}", api_url, shell_name)).unwrap();
    
    let mut login_code = String::new();
    println!("Enter the login code that shows on the hoposhell GUI: ");
    std::io::stdin().read_line(&mut login_code).unwrap();
    let credentials = reqwest::blocking::get(format!("{}/shell-credentials/confirmation/{}/{}", api_url, shell_name, login_code)).unwrap()
        .json::<HashMap<String, String>>().unwrap();

    let server_crt = &credentials["serverCrt"];
    let shell_key = &credentials["shellKey"];

    let server_crt_folder_path = Path::new(&server_crt_path).parent().unwrap();
    if !server_crt_folder_path.exists() {
        println!("💾 Create folder {}", server_crt_folder_path.to_str().unwrap());
        std::fs::create_dir_all(server_crt_folder_path).unwrap();
    }
    println!("💾 Write server crt in file {}", server_crt_path);
    std::fs::write(&server_crt_path, server_crt).expect("Unable to write server crt file");
    
    
    let shell_key_folder_path = Path::new(&shell_key_path).parent().unwrap();
    if !shell_key_folder_path.exists() {
        println!("💾 Create folder {}", shell_key_folder_path.to_str().unwrap());
        std::fs::create_dir_all(shell_key_folder_path).unwrap();
    }
    println!("💾 Write shell key in file {}", shell_key_path);
    std::fs::write(&shell_key_path, shell_key).expect("Unable to write shell key file");
    
    // println!("💾 Prepare hopo command {}", shell_key_path);
    // let hoposhell_folder_path = Path::new(&hoposhell_folder_path);
    // if !hoposhell_folder_path.exists() {
    //     println!("💾 Create folder {}", hoposhell_folder_path.to_str().unwrap());
    //     std::fs::create_dir_all(hoposhell_folder_path).unwrap();
    // }
    // let hoposhell_exe_path =  std::env::current_exe().unwrap();
    // std::fs::copy(hoposhell_exe_path, hoposhell_folder_path.join("hopo")).unwrap();
}

struct ShellIdAndPath {
    shell_id: String,
    path: String
}

fn split_shell_id_and_path(p: &String) -> Option<ShellIdAndPath> {
    let splitted_p = p.split(":").collect::<Vec<&str>>();
    if splitted_p.len() != 2 {
        return None;
    } else {
        return Some(ShellIdAndPath {
            shell_id: splitted_p[0].to_string(),
            path: splitted_p[1].to_string()
        });
    }
}

fn make_random_id(n: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(n)
        .map(char::from)
        .collect::<String>()
}

enum ParseCommandResponseResult {
    CanContinue,
    ReachedLastChunk,
    Error
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
        if message.content.is_none() {
            eprintln!("Got an empty message: ignore");
            return ParseCommandResponseResult::CanContinue;
        }

        // TODO: avoid clone
        let content: Vec<u8> = message.content.clone().unwrap();

        match ChunkedRequestOrResponse::deserialize(&content) {
            ChunkedRequestOrResponse::Request(req_or_res) => {
                eprint!("Got a request, but was waiting for a resposnse: ignore...");
                return ParseCommandResponseResult::CanContinue;
            },
            ChunkedRequestOrResponse::None => {
                eprintln!("Got an empty command message: ignore");
            return ParseCommandResponseResult::CanContinue;
            },
            ChunkedRequestOrResponse::Response(res) => {
                if res.message_id != req.message_id {
                    eprintln!("Got a response with a different message_id: ignore");
                    return ParseCommandResponseResult::CanContinue;
                }
                if res.status_code != StatusCode::Ok {
                    eprintln!("Got a response with status {:?}: exit", res.status_code);
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