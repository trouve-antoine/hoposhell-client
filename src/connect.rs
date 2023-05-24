use std::{
    io::{self, Read, Write},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
    net::{TcpStream},
    sync::mpsc::{self, Sender, Receiver},
    fs
};

use base64::engine::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

use regex::Regex;

use expect_exit::{Expected};

use openssl::{ssl::{self, SslConnector, SslFiletype}};
use crate::commands::resize::make_size_message;

use super::constants::BUF_SIZE;

use super::message::{Message, MessageTypeToCmd, MessageTypeToStream, separate_messages};
use super::run_shell::run_shell;
use super::args::Args;

fn get_now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}


pub fn make_ssl_conector(server_crt_path: &String, shell_key_path: &String, verify_crt: bool) -> SslConnector {
    // Configure OpenSSL
    eprintln!("Use server certificate at {}", server_crt_path);
    eprintln!("Use shell key at {}", shell_key_path);

    let mut ssl_builder = ssl::SslConnector::builder(ssl::SslMethod::tls_client()).unwrap();
    
    ssl_builder.set_ca_file(&server_crt_path).expect_or_exit(||
        format!("Unable to load Hoposhell server certificate at {}. You might need to run command `hopo setup`.", &server_crt_path)
    );
    
    ssl_builder.set_certificate_file(&shell_key_path, SslFiletype::PEM).expect_or_exit(||
        format!("Unable to load shell certificate at {}. You might need to run command `hopo setup`.", &shell_key_path)
    );
    ssl_builder.set_private_key_file(&shell_key_path, SslFiletype::PEM).expect_or_exit(||
        format!("Unable to load shell private key at {}. You might need to run command `hopo setup`", &shell_key_path)
    );

    if !verify_crt {
        eprintln!("!! I will not verify the server CRT");
        ssl_builder.set_verify(ssl::SslVerifyMode::NONE);
    }
    return ssl_builder.build();
}

pub fn compute_hostname(server_url: &String) -> &str {
    let parts: Vec<&str> = server_url.split(":").collect();
    return parts[0];
}

pub fn main_connect(args: Args) {
    let mut args = args.clone();

    if args.shell_key_path.is_none() {
        // Pick up the first shell key in the folder
        let shell_pem_regex = Regex::new("shell_.*\\.pem").unwrap();
        let all_files = list_files_in_folder(&args.hoposhell_folder_path, &shell_pem_regex);
        match all_files.len() {
            0 => {
                eprintln!("There are no shell certificate in folder {}", &args.hoposhell_folder_path);
            },
            1 => {
                eprintln!("Using default shell key in folder {}: {}", &args.hoposhell_folder_path, all_files[0]);
                args.shell_key_path = Some(all_files[0].clone());
            }
            _ => {
                eprintln!("There are more than on shell certificate in folder {}. Cannot determine default.", &args.hoposhell_folder_path);
            }
        }
    }

    let (tx_to_cmd, rx_cmd) = mpsc::channel::<Message<MessageTypeToCmd>>();
    let tx_to_cmd = Arc::new(Mutex::new(tx_to_cmd));
    let rx_cmd = Arc::new(Mutex::new(rx_cmd));
    
    let (tx_to_stream, rx_stream) = mpsc::channel::<Message<MessageTypeToStream>>();
    let tx_to_stream = Arc::new(Mutex::new(tx_to_stream));
    let rx_stream = Arc::new(Mutex::new(rx_stream));
    
    let history_of_messages_to_stream: Vec<Message<MessageTypeToStream>> = vec![];
    let history_of_messages_to_stream = Arc::new(Mutex::new(history_of_messages_to_stream));

    let rx_cmd = Arc::clone(&rx_cmd);
    let tx_to_stream = Arc::clone(&tx_to_stream);
    let master_pty = run_shell(
        args.get_shell_id(),
        &args.hoposhell_folder_path,
        &args.cmd,
        args.default_cols, args.default_rows,
        tx_to_stream, rx_cmd,
        history_of_messages_to_stream.clone()
    );

    if let Err(_) = &master_pty {
        eprintln!("Unable to run the shell");
    }

    let master_pty = master_pty.unwrap();

    let hostname = compute_hostname(&args.server_url);

    let ssl_connector = if args.use_ssl {
        Some(make_ssl_conector(
            &args.server_crt_path.expect_or_exit(|| format!("Please specify env var HOPOSHELL_SERVER_CRT, or run `hopo setup` to download it to the default location.")),
            &args.shell_key_path.expect_or_exit(|| format!("Please specify env var HOPOSHELL_SHELL_KEY, or specify the shell_id parameter of the connect command.")),
            args.verify_crt
        ))
    } else {
        None
    };
    
    loop {
        eprintln!("Tries to connect to: {}", args.server_url);
        match TcpStream::connect(args.server_url.as_str()) {
            Ok(tcp_stream) => {
                eprintln!("Connected to server");

                tcp_stream.set_read_timeout(Some(args.read_timeout)).expect("Could not set the read timeout of the tcp stream");

                if let Some(ref ssl_connector) = ssl_connector {
                    let ssl_stream = ssl_connector.connect(hostname, tcp_stream).unwrap();
                    handle_connection(
                        ssl_stream,
                        tx_to_cmd.clone(), rx_stream.clone(),
                        &args.version,
                        history_of_messages_to_stream.clone(),
                        master_pty.clone(),
                        args.keep_alive,
                        args.read_timeout_sleep,
                        args.verbose
                    )
                } else {
                    handle_connection(
                        tcp_stream, tx_to_cmd.clone(), rx_stream.clone(),
                        &args.version,
                        history_of_messages_to_stream.clone(),
                        master_pty.clone(),
                        args.keep_alive, args.read_timeout_sleep,
                        args.verbose
                    );
                }

                eprintln!("Lost connection.");

                if !args.auto_reconnect {
                    break
                }
            }
            Err(e) => {
                eprintln!("Failed to connect {:?}", e);
                thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn handle_connection(
    mut stream: impl Read + Write,
    tx_to_cmd: Arc<Mutex<Sender<Message<MessageTypeToCmd>>>>,
    rx_stream: Arc<Mutex<Receiver<Message<MessageTypeToStream>>>>,
    version: &String,
    history_of_messages_to_stream: Arc<Mutex<Vec<Message<MessageTypeToStream>>>>,
    master_pty: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    keep_alive_delta: Duration,
    read_timeout_sleep: Duration,
    verbose: bool
) {
    let header_msg = Message {
        mtype: MessageTypeToStream::HEADER,
        content: Some(format!("v{}", version).as_bytes().to_vec())
    };
    if let Err(_) = send_message_to_stream(&header_msg, &mut stream, verbose) {
        eprintln!("Unable to send headers to stream...");
        return;
    }

    if let Err(_) = send_message_to_stream(&make_size_message(&master_pty.lock().unwrap()), &mut stream, verbose) {
        eprintln!("Unable to send headers to stream...");
        return;
    }

    for msg in history_of_messages_to_stream.lock().unwrap().iter() {
        match send_message_to_stream(&msg, &mut stream, verbose) {
            Ok(_) => { }
            Err(e) => {
                eprint!("Got an error while writing history to stream: {:?}", e);
                return;
            }
        }
    }

    let mut buf_str = String::from("");

    let mut last_keep_alive: Option<u128> = None;

    let keep_alive_payload = "---\n".as_bytes();

    loop {
        /* Try to read */
        match read_messages_from_stream(&mut stream, &mut buf_str, verbose) {
            ReadMessageResult::Ok(messages) => {
                for message in messages.iter() {
                    tx_to_cmd.lock().unwrap().send(message.clone()).unwrap();
                }
            },
            ReadMessageResult::CanContinue => {
                if read_timeout_sleep.as_millis() > 0 {
                    eprint!("*");
                    thread::sleep(read_timeout_sleep);
                }
            },
            ReadMessageResult::CannotContinue => {
                break;
            }
        }

        /* Send output to stream if any */
        match rx_stream.lock().unwrap().recv_timeout(Duration::from_millis(100)) {
            Ok(msg) => {
                match send_message_to_stream(&msg, &mut stream, verbose){
                    Ok(_) => { }
                    Err(e) => {
                        eprintln!("Got an error while writing content to stream: {:?}", e);
                        return;
                    }
                }
            },
            Err(e) => {
                if e != mpsc::RecvTimeoutError::Timeout {
                    eprintln!("The message channel from the command has been closed: {:?}", e);
                    break;
                }
            }
        }
        /* Keep Alive */
        let now = get_now();
        if let Some(last_keep_alive) = last_keep_alive {
            if now - last_keep_alive < keep_alive_delta.as_millis() {
                continue;
            }
        }

        if verbose {}
            eprintln!("-- send keep alive message to tcp stream with size: {}", keep_alive_payload.len());
    }
        
        match stream.write(keep_alive_payload) {
            Ok(_) => {}
            Err(e) => {
                eprint!("Got an error while writing in keep alive: {:?}", e);
                break;
            }
        }
        match stream.flush() {
            Ok(_) => {}
            Err(e) => {
                eprint!("Got an error while flushing in keep alive: {:?}", e);
                break;
            }
        }
        last_keep_alive = Some(now);
    }

    eprintln!("Got disconnected from server.");    
}

pub enum ReadMessageResult {
    Ok(Vec<Message<MessageTypeToCmd>>),
    CanContinue,
    CannotContinue
}

pub fn read_messages_from_stream(
    mut stream: impl Read + Write,
    mut buf_str: &mut String,
    verbose: bool
) -> ReadMessageResult {
    let mut buf = [0u8; BUF_SIZE];
    match stream.read(&mut buf) {
        Ok(n) => {
            if n == 0 {
                eprintln!("Close stream read thread: the socket has been closed.");
                return ReadMessageResult::CannotContinue;
            }

            if verbose {
                eprintln!("-- got {} bytes from stream.", n);
            }
            
            return ReadMessageResult::Ok(
                separate_messages(&mut buf_str, &buf, n)
            );
        }
        Err(e) => {
            match e.kind() {
                io::ErrorKind::WouldBlock => {
                    return ReadMessageResult::CanContinue;
                }
                _ => {
                    eprintln!("Got an error while reading the TCP stream -- {:?}", e);
                    return ReadMessageResult::CannotContinue;
                }
            }
        }
    }
}

pub fn send_message_to_stream(msg: &Message<MessageTypeToStream>, stream_writer: &mut impl Write, verbose: bool) -> io::Result<usize> {
    match &msg.content {
        None => { return Ok(0) }
        Some(content) => {
            let content_64 = BASE64.encode(content);
            
            let encoded_content = match msg.mtype {
                // MessageTypeToStream::STDERR => format!("{}-eee---\n", content_64),
                MessageTypeToStream::STDOUT => format!("{}-ooo---\n", content_64),
                MessageTypeToStream::HEADER => format!("{}-hhh---\n", content_64),
                MessageTypeToStream::COMMAND => format!("{}-ccc---\n", content_64)
            };
            let encoded_content = encoded_content.as_bytes();
            if verbose {
                eprintln!("-- send message to tcp stream with size: {}", encoded_content.len());
            }
            return stream_writer.write(encoded_content);
        }
    }
}

fn list_files_in_folder(path: &String, file_pattern: &Regex) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    let paths = fs::read_dir(path).unwrap();
    for path in paths {
        let path = path.unwrap().path();
        let path_str = path.to_str().unwrap();
        if file_pattern.is_match(path_str) {
            files.push(String::from(path_str));
        }
    }
    return files;
}