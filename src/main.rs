use portable_pty as pty;

use std::{
    path::Path,
    env,
    thread,
    io::{self, Read, Write},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
    net::{TcpStream},
    sync::mpsc::{self, Sender, Receiver}
};

use openssl::{ssl::{self, SslConnector, SslFiletype}};

#[derive(Clone, PartialEq, Debug, Copy)]
enum MessageTypeToCmd {
    STDIN, COMMAND
}
#[derive(Clone, PartialEq, Debug, Copy)]
enum MessageTypeToStream {
    STDOUT, HEADER
    // STDERR, 
}

const BUF_SIZE: usize = 1024;

const HOPOSHELL_FOLDER_NAME: &str = ".hoposhell";

#[derive(Clone, Debug)]
struct Message<T> {
    mtype: T,
    // content: Option<[u8; BUF_SIZE]>
    content: Option<Vec<u8>>
}

fn get_now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

enum ArgsCommand {
    CONNECT, SETUP
}

struct Args {
    use_ssl: bool,
    cmd: String,
    auto_reconnect: bool,
    server_url: String,
    keep_alive: Duration,
    read_timeout: Duration,
    read_timeout_sleep: Duration,
    server_crt_path: Option<String>,
    shell_key_path: Option<String>,
    verify_crt: bool,
    command: ArgsCommand,
    shell_name: Option<String>
}

fn parse_duration_from_ms_str(time_ms_str: String) -> Duration {
    let time_ms: u64 = time_ms_str.parse().unwrap();
    return Duration::from_millis(time_ms);

}

fn parse_args() -> Args {
    let cmd_args: Vec<String> = env::args().collect();

    let mut shell_name: Option<String> = None;
    let mut command = ArgsCommand::CONNECT;
    if cmd_args.len() > 1 {    
        (shell_name, command) = match cmd_args[1].as_str() {
            "connect" => (Some(cmd_args[2].clone()), ArgsCommand::CONNECT),
            "setup" => (Some(cmd_args[2].clone()), ArgsCommand::SETUP),
            _ => (Some(cmd_args[1].clone()), ArgsCommand::CONNECT)
        };
    }

    let hoposhell_folder_path = Path::new(&env::var("HOME").unwrap()).join(HOPOSHELL_FOLDER_NAME);

    let mut args = Args {
        auto_reconnect: false,
        cmd: match env::var("SHELL") {
            Ok(x) => x,
            Err(_) => String::from("bash")
        },
        use_ssl: true,
        server_url: String::from("api.hoposhell.com:10000"),
        keep_alive:Duration::from_millis(5000),
        read_timeout: Duration::from_millis(50),
        read_timeout_sleep: Duration::ZERO,
        server_crt_path: Some(String::from(hoposhell_folder_path.join("server.crt").to_str().unwrap())),
        shell_key_path: if let Some(shell_name) = shell_name.as_ref() {
            Some(format!("{}/{}.pem", hoposhell_folder_path.to_str().unwrap(), shell_name))
        } else { None },
        verify_crt: true,
        command: command,
        shell_name: shell_name
    };

    let reconnect_str = env::var("RECONNECT");
    if let Ok(reconnect_str) = reconnect_str {
        args.auto_reconnect = match reconnect_str.to_lowercase().as_str() {
            "no" | "false" | "0" => false,
            _ => true
        };
    }

    let use_ssl_str = env::var("USE_SSL");
    if let Ok(use_ssl_str) = use_ssl_str {
        args.use_ssl = match use_ssl_str.to_lowercase().as_str() {
            "no" | "false" | "0" => false,
            _ => true
        };
    }

    let server_url = env::var("HOPOSHELL_URL");
    if let Ok(server_url) = server_url {
        args.server_url = server_url;
    }

    let keep_alive_ms_str = env::var("KEEP_ALIVE");
    if let Ok(keep_alive_ms_str) = keep_alive_ms_str {
        args.keep_alive = parse_duration_from_ms_str(keep_alive_ms_str);
    }
    
    let read_timeout_ms_str = env::var("READ_TIMEOUT");
    if let Ok(read_timeout_ms_str) = read_timeout_ms_str {
        args.read_timeout = parse_duration_from_ms_str(read_timeout_ms_str);
    }
    
    let read_timeout_sleep_str = env::var("READ_TIMEOUT_SLEEP");
    if let Ok(read_timeout_sleep_str) = read_timeout_sleep_str {
        args.read_timeout_sleep = parse_duration_from_ms_str(read_timeout_sleep_str);
    }
    
    let server_crt_path_str = env::var("HOPOSHELL_SERVER_CRT");
    if let Ok(server_crt_path_str) = server_crt_path_str {
        args.server_crt_path = Some(server_crt_path_str);
    }
    
    let shell_key_path_str = env::var("HOPOSHELL_SHELL_KEY");
    if let Ok(shell_key_path_str) = shell_key_path_str {
        args.shell_key_path = Some(shell_key_path_str);
    }

    let verify_crt_str = env::var("VERIFY_CRT");
    if let Ok(verify_crt_str) = verify_crt_str {
        args.verify_crt = match verify_crt_str.to_lowercase().as_str() {
            "no" | "false" | "0" => false,
            _ => true
        };
    }

    return args;
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

fn main() {
    let args = parse_args();

    match args.command {
        ArgsCommand::CONNECT => main_connect(args),
        ArgsCommand::SETUP => main_setup(args)
    }
}

fn main_setup(args: Args) {
    /* */
    match args.shell_name {
        Some(shell_name) => {
            println!("Get credentials for shell {}", shell_name);
        },
        None => {
            eprintln!("Please specify the shell name");
        }
    }    
}

fn main_connect(args: Args) {
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
    let _command = run_command(args.cmd, tx_to_stream, rx_cmd, history_of_messages_to_stream.clone());

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
                        history_of_messages_to_stream.clone(), 
                        args.keep_alive,
                        args.read_timeout_sleep
                    )
                } else {
                    handle_connection(
                        tcp_stream, tx_to_cmd.clone(), rx_stream.clone(),
                        history_of_messages_to_stream.clone(),
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

fn run_command(
    cmd: String,
    tx_to_stream: Arc<Mutex<Sender<Message<MessageTypeToStream>>>>,
    rx_cmd: Arc<Mutex<Receiver<Message<MessageTypeToCmd>>>>,
    history_of_messages_to_stream: Arc<Mutex<Vec<Message<MessageTypeToStream>>>>) -> io::Result<()>
{
    let pty_system = pty::native_pty_system();

    let pty_pair = pty_system.openpty(pty::PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    }).unwrap();

    let cmd = pty::CommandBuilder::new(cmd);

    let _pty_child = pty_pair.slave.spawn_command(cmd).expect("Unable to spawn shell");
    
    let reader = pty_pair.master.try_clone_reader().expect("Unable to get a pty reader");
    let writer = pty_pair.master.try_clone_writer().expect("Unable to get a writer");
    
    let cmd_stdout = Arc::new(Mutex::new(reader));
    let cmd_stdin = Arc::new(Mutex::new(writer));

    let _stdin_handle = thread::spawn(move || loop {
        if let Ok(msg) = rx_cmd.lock().unwrap().recv() {
            if msg.mtype == MessageTypeToCmd::STDIN {
                if let Some(content) = msg.content {
                    cmd_stdin.lock().unwrap().write(&content).unwrap();
                }
            } else if msg.mtype == MessageTypeToCmd::COMMAND {
                let content = match msg.content.as_deref() {
                    Some(c) => Some(String::from_utf8_lossy(&c)),
                    None => None
                };
                match content.as_deref() {
                    Some("restart") => { eprintln!("Got restart command"); std::process::exit(0) },
                    Some(c) => eprintln!("Got unknown command: {}", c),
                    None => eprintln!("Got an empty command.")
                }
            } else {
                eprintln!("Unknown message: {:?}", msg);
            }
        };
    });

    let tx_to_stream_stdout = Arc::clone(&tx_to_stream);
    let history_of_messages_to_stream_stdout = history_of_messages_to_stream.clone();
    let _stdout_handle = thread::spawn(move || loop {
        let mut buf = [0u8; BUF_SIZE];
        match cmd_stdout.lock().unwrap().read(&mut buf) {
            Ok(n) => {
                if n == 0 {
                    eprintln!("The command died...");
                    std::process::exit(0);
                }
                let msg = Message {
                    mtype: MessageTypeToStream::STDOUT,
                    content: Some(buf.to_vec())
                };
                history_of_messages_to_stream_stdout.lock().unwrap().push(msg.clone());
                tx_to_stream_stdout.lock().unwrap().send(msg).unwrap();
            }
            Err(e) => {
                eprintln!("Got an error wile reading stdout: {:?}", e);
                break;
            }
        };
    });

    return Ok(());
}

fn handle_connection(
    mut stream: impl Read + Write,
    tx_to_cmd: Arc<Mutex<Sender<Message<MessageTypeToCmd>>>>,
    rx_stream: Arc<Mutex<Receiver<Message<MessageTypeToStream>>>>,
    history_of_messages_to_stream: Arc<Mutex<Vec<Message<MessageTypeToStream>>>>,
    keep_alive_delta: Duration,
    read_timeout_sleep: Duration)
{
    let header_msg = Message {
        mtype: MessageTypeToStream::HEADER,
        content: Some("v1.0".as_bytes().to_vec())
    };
    if let Err(_) = send_message_to_stream(&header_msg, &mut stream) {
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
                MessageTypeToStream::HEADER => format!("{}-hhh---\n", content_64)
            };
            return stream_writer.write(encoded_content.as_bytes());
        }
    }
}

fn separate_messages(buffer: &mut String, new_data: &[u8; BUF_SIZE], n: usize) -> Vec<Message<MessageTypeToCmd>> {
    buffer.push_str(std::str::from_utf8(&new_data[..n]).unwrap());
            
    let buffer_copy = buffer.clone();
    let buffer_parts: Vec<&str> = buffer_copy.split("---\n").collect();
    buffer.clear();

    let mut messages = vec![];

    for buf_part in buffer_parts.iter() {
        if buf_part.len() == 0 { continue; }

        let is_last = buf_part == buffer_parts.last().unwrap();

        let mtype: Option<MessageTypeToCmd> = if buf_part.ends_with("-iii") {
            Some(MessageTypeToCmd::STDIN)
        } else if buf_part.ends_with("-ccc") {
            Some(MessageTypeToCmd::COMMAND)
        } else {
            None
        };

        match mtype {
            None => {
                if is_last { buffer.push_str(buf_part); }
                else { eprintln!("\n[EE] Got bad part in communication"); }
            }
            Some(mtype) => {
                // 4 is the length of -eee or -ooo
                let payload_64: &str = &buf_part[..buf_part.len()-4];
                let payload = base64::decode(payload_64).unwrap();
                messages.push(Message { mtype, content: Some(payload) });
            }
        }
    }
    return messages;
}