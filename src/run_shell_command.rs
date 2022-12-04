use std::{
    sync::{Mutex, Arc},
    sync::mpsc::{Sender, Receiver},
    io,
    thread
};

use portable_pty as pty;

use super::message::{Message, MessageTypeToCmd, MessageTypeToStream, make_size_message};

const BUF_SIZE: usize = 1024;
const MAX_MESSAGE_HISTORY_SIZE: usize = 2048;

pub fn run_command(
    shell_id: Option<String>,
    _hoposhell_folder: String,
    cmd: String,
    cols: u16,
    rows: u16,
    tx_to_stream: Arc<Mutex<Sender<Message<MessageTypeToStream>>>>,
    rx_cmd: Arc<Mutex<Receiver<Message<MessageTypeToCmd>>>>,
    history_of_messages_to_stream: Arc<Mutex<Vec<Message<MessageTypeToStream>>>>
) -> io::Result<Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>>
{
    let pty_system = pty::native_pty_system();

    let pty_pair = pty_system.openpty(pty::PtySize {
        rows: rows,
        cols: cols,
        pixel_width: 0,
        pixel_height: 0,
    }).unwrap();

    let mut cmd = pty::CommandBuilder::new(cmd);
    
    /* Update env vars for child shell */
    if let Some(shell_id) = shell_id {
        cmd.env("HOPOSHELL_SHELL_ID", &shell_id);
        cmd.env("HOPOSHELL_CONNECTED", &shell_id);
    } else {
        cmd.env("HOPOSHELL_CONNECTED", "1");
    }

    let _pty_child = pty_pair.slave.spawn_command(cmd).expect("Unable to spawn shell");

    let reader = pty_pair.master.try_clone_reader().expect("Unable to get a pty reader");
    let writer = pty_pair.master.try_clone_writer().expect("Unable to get a writer");
    
    let master = Arc::new(Mutex::new(pty_pair.master));
    
    let cmd_stdout = Arc::new(Mutex::new(reader));
    let cmd_stdin = Arc::new(Mutex::new(writer));

    let tx_to_stream_stdin = Arc::clone(&tx_to_stream);

    let master_stdin = master.clone();
    let _stdin_handle = thread::spawn(move || loop {
        if let Ok(msg) = rx_cmd.lock().unwrap().recv() {
            if msg.mtype == MessageTypeToCmd::STDIN {
                if let Some(content) = msg.content {
                    cmd_stdin.lock().unwrap().write(&content).unwrap();
                }
            } else if msg.mtype == MessageTypeToCmd::COMMAND {
                match msg.content.as_deref() {
                    Some(c) => {
                        let c = String::from_utf8_lossy(&c);
                        let mut parts =  c.split("/");
                        let cmd = parts.next();
                        match cmd {
                            Some("restart") => { eprintln!("Got restart command"); std::process::exit(0) },
                            Some("resize") => {
                                let rows: Option<u16> = parse_next_int(&mut parts);
                                let cols: Option<u16> = parse_next_int(&mut parts);

                                if let (Some(rows), Some(cols)) = (rows, cols) {
                                    let pty_size = pty::PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
                                    let master = master_stdin.lock().unwrap();
                                    if let Ok(_) = master.resize(pty_size) {
                                        tx_to_stream_stdin.lock().unwrap().send(make_size_message(&master)).unwrap();
                                    } else {
                                        eprintln!("Unable to set terminal size");
                                    }
                                } else {
                                   eprintln!("Got incorrect resize command: {:?}", c); 
                                }
                            },
                            _ => { eprintln!("Got unknown command: {:?}", cmd); }
                        }
                    }
                    None => {
                        eprintln!("Got an empty command.")
                    }
                };
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
                    content: Some(buf[..n].to_vec())
                };
                {
                    let mut history_of_messages = history_of_messages_to_stream_stdout.lock().unwrap();
                    if history_of_messages.len() > MAX_MESSAGE_HISTORY_SIZE {
                        let delta = history_of_messages.len()-MAX_MESSAGE_HISTORY_SIZE;
                        history_of_messages.drain(0..delta);
                    }
                    history_of_messages.push(msg.clone());
                }

                tx_to_stream_stdout.lock().unwrap().send(msg).unwrap();
            }
            Err(e) => {
                eprintln!("Got an error wile reading stdout: {:?}", e);
                break;
            }
        };
    });

    return Ok(master);
}

fn parse_next_int(parts: &mut std::str::Split<&str>) -> Option<u16> {
    if let Some(rows_str) = parts.next() {
        return match rows_str.parse() {
            Ok(rows) => Some(rows),
            Err(_) => None
        };
    } else {
        return None;
    };
}