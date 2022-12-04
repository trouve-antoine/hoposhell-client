use std::{
    io::{self, Read, Write},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
    net::{TcpStream},
    sync::mpsc::{self, Sender, Receiver}
};

use openssl::{ssl::{self, SslConnector, SslFiletype}};
use super::constants::BUF_SIZE;

use super::message::{Message, MessageTypeToCmd, MessageTypeToStream, make_size_message, separate_messages};
use super::run_shell_command::{run_command};
use super::args::Args;

fn get_now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}


fn make_ssl_conector(server_crt_path: String, shell_key_path: String, verify_crt: bool) -> SslConnector {
    // Configure OpenSSL
    println!("Use server certificate at {}", server_crt_path);
    println!("Use shell key at {}", shell_key_path);

    let mut ssl_builder = ssl::SslConnector::builder(ssl::SslMethod::tls_client()).unwrap();
    ssl_builder.set_ca_file(server_crt_path).unwrap();
    
    ssl_builder.set_certificate_file(&shell_key_path, SslFiletype::PEM).unwrap();
    ssl_builder.set_private_key_file(&shell_key_path, SslFiletype::PEM).unwrap();

    if !verify_crt {
        println!("!! I will not verify the server CRT");
        ssl_builder.set_verify(ssl::SslVerifyMode::NONE);
    }
    return ssl_builder.build();
}

fn compute_hostname(server_url: &String) -> &str {
    let parts: Vec<&str> = server_url.split(":").collect();
    return parts[0];
}

pub fn main_connect(args: Args) {
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
    let master_pty = run_command(
        args.shell_name,
        args.hoposhell_folder_path,
        args.cmd,
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
            args.server_crt_path.expect("Please specify env var HOPOSHELL_SERVER_CRT"),
            args.shell_key_path.expect("Please specify env var HOPOSHELL_SHELL_KEY"),
             args.verify_crt
        ))
    } else {
        None
    };
    
    loop {
        println!("Tries to connect to: {}", args.server_url);
        match TcpStream::connect(args.server_url.as_str()) {
            Ok(tcp_stream) => {
                println!("Connected to server");

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
                        args.read_timeout_sleep
                    )
                } else {
                    handle_connection(
                        tcp_stream, tx_to_cmd.clone(), rx_stream.clone(),
                        &args.version,
                        history_of_messages_to_stream.clone(),
                        master_pty.clone(),
                        args.keep_alive, args.read_timeout_sleep
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
    read_timeout_sleep: Duration)
{
    let header_msg = Message {
        mtype: MessageTypeToStream::HEADER,
        content: Some(format!("v{}", version).as_bytes().to_vec())
    };
    if let Err(_) = send_message_to_stream(&header_msg, &mut stream) {
        eprintln!("Unable to send headers to stream...");
        return;
    }

    if let Err(_) = send_message_to_stream(&make_size_message(&master_pty.lock().unwrap()), &mut stream) {
        eprintln!("Unable to send headers to stream...");
        return;
    }

    for msg in history_of_messages_to_stream.lock().unwrap().iter() {
        match send_message_to_stream(&msg, &mut stream) {
            Ok(_) => { }
            Err(e) => {
                eprint!("Got an error while writing history to stream: {:?}", e);
                return;
            }
        }
    }

    let mut buf_str = String::from("");

    let mut last_keep_alive: Option<u128> = None;

    loop {
        /* Try to read */
        let mut buf = [0u8; BUF_SIZE];
        match stream.read(&mut buf) {
            Ok(n) => {
                if n == 0 {
                    eprintln!("Close stream read thread: the socket has been closed.");
                    break;
                }
                
                let messages = separate_messages(&mut buf_str, &buf, n);

                for message in messages.iter() {
                    tx_to_cmd.lock().unwrap().send(message.clone()).unwrap();
                }
            }
            Err(e) => {
                match e.kind() {
                    io::ErrorKind::WouldBlock => {
                        if read_timeout_sleep.as_millis() > 0 {
                            eprint!("*");
                            thread::sleep(read_timeout_sleep);
                        }
                    }
                    _ => {
                        eprintln!("Got an error while reading the TCP stream -- {:?}", e);
                        break;        
                    }
                }
            }
        }
        /* Send output to stream if any */
        match rx_stream.lock().unwrap().recv_timeout(Duration::from_millis(100)) {
            Ok(msg) => {
                match send_message_to_stream(&msg, &mut stream){
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
        
        match stream.write("---\n".as_bytes()) {
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
        println!("[keep alive]")
    }

    println!("Got disconnected from server.");    
}

fn send_message_to_stream(msg: &Message<MessageTypeToStream>, stream_writer: &mut impl Write) -> io::Result<usize> {
    match &msg.content {
        None => { return Ok(0) }
        Some(content) => {
            let content_64 = base64::encode(content);
            
            let encoded_content = match msg.mtype {
                // MessageTypeToStream::STDERR => format!("{}-eee---\n", content_64),
                MessageTypeToStream::STDOUT => format!("{}-ooo---\n", content_64),
                MessageTypeToStream::HEADER => format!("{}-hhh---\n", content_64),
                MessageTypeToStream::COMMAND => format!("{}-ccc---\n", content_64)
            };
            return stream_writer.write(encoded_content.as_bytes());
        }
    }
}