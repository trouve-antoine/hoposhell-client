use super::constants::{BUF_SIZE};
use base64::engine::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

#[derive(Clone, PartialEq, Debug, Copy)]
pub enum MessageTypeToCmd {
    STDIN, COMMAND
}
#[derive(Clone, PartialEq, Debug, Copy)]
pub enum MessageTypeToStream {
    STDOUT, HEADER, COMMAND
    // STDERR, 
}

#[derive(Clone, Debug)]
pub struct Message<T> {
    pub mtype: T,
    pub content: Option<Vec<u8>>
}

pub fn separate_messages(buffer: &mut String, new_data: &[u8; BUF_SIZE], n: usize) -> Vec<Message<MessageTypeToCmd>> {
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
                let payload = BASE64.decode(payload_64).unwrap();
                messages.push(Message { mtype, content: Some(payload) });
            }
        }
    }
    return messages;
}
