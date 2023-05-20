use std::{
    sync::{Mutex, Arc},
    sync::mpsc::{Sender, Receiver},
    io,
    thread
};

use portable_pty as pty;

use super::message::{Message, MessageTypeToCmd, MessageTypeToStream};
use super::constants::{BUF_SIZE, MAX_MESSAGE_HISTORY_SIZE};

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
    let _stdin_handle = thread::spawn(move || {
        let mut commands = crate::commands::command_processor::CommandProcessor::new();

        loop {
            if let Ok(msg) = rx_cmd.lock().unwrap().recv() {
                if msg.mtype == MessageTypeToCmd::STDIN {
                    if let Some(content) = msg.content {
                        cmd_stdin.lock().unwrap().write(&content).unwrap();
                    }
                } else if msg.mtype == MessageTypeToCmd::COMMAND {
                    match msg.content {
                        Some(c) => {
                            /******* */
                            let send_message = |msg: Message<MessageTypeToStream>| {
                                tx_to_stream_stdin.lock().unwrap().send(msg).unwrap();
                            };
                            /******* */
                            if c.starts_with(b"restart") {
                                /* The shell will die */
                                crate::commands::restart::process_restart_command();
                            } else if c.starts_with(b"resize") {
                                /* The shell will resize, and sends its current size back */
                                crate::commands::resize::process_resize_command(&c, &master_stdin, &send_message);
                            } else {
                                /* A generic command */
                                if let Some(res) = commands.process_msg(&c) {
                                    /* consume and send the response back */
                                    eprintln!("Got a command: {:?}", res.cmd);
                                    for chunk in res.chunk() {
                                        let msg = Message {
                                            mtype: MessageTypeToStream::COMMAND,
                                            content: Some(chunk.to_message_payload())
                                        };
                                        send_message(msg);
                                    }
                                } else {
                                    eprintln!("Got an invalid command.");
                                }
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
        }
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