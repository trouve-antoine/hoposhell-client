use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use portable_pty as pty;

use crate::constants::{MESSAGE_PARTS_SEPARATOR};
use crate::message::{Message, MessageTypeToStream};

pub fn process_resize_command(
    c: &Cow<&[u8]>,
    master_stdin: &Arc<Mutex<Box<dyn pty::MasterPty + Send>>>,
    send_message: &dyn Fn(Message<MessageTypeToStream>)
) {
    /* The shell will resize, and sends its current size back */
    let mut parts = c.splitn(3, |x| x == &MESSAGE_PARTS_SEPARATOR);
    
    let cmd = parts.next();
    let rows = maybe_u16(parts.next());
    let cols = maybe_u16(parts.next());

    if let (Some(rows), Some(cols)) = (rows, cols) {
        let pty_size = pty::PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
        let master = master_stdin.lock().unwrap();
        if let Ok(_) = master.resize(pty_size) {
            send_message(make_size_message(&master));
        } else {
            eprintln!("Unable to set terminal size");
        }
    } else {
        eprintln!("Got incorrect resize command: {:?}", c); 
    }
}

fn maybe_u16(v: Option<&[u8]>) -> Option<u16> {
    match v {
        Some(v) => match String::from_utf8(v.to_vec()) {
            Ok(v) => match v.parse::<u16>() {
                Ok(int_res) => Some(int_res),
                Err(_) => None
            },
            Err(_) => None
        },
        None => None
    }
}

pub fn make_size_message(master_pty: &Box<dyn portable_pty::MasterPty + Send>) -> Message<MessageTypeToStream>{
    let pty_size = master_pty.get_size().unwrap();

    return Message {
        mtype: MessageTypeToStream::COMMAND,
        content: Some(format!("size{sep}{r}{sep}{c}", r=pty_size.rows, c=pty_size.cols, sep=MESSAGE_PARTS_SEPARATOR).as_bytes().to_vec())
    };
}