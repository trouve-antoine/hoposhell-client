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

pub fn make_size_message(master_pty: &Box<dyn portable_pty::MasterPty + Send>) -> Message<MessageTypeToStream>{
    let pty_size = master_pty.get_size().unwrap();

    return Message {
        mtype: MessageTypeToStream::COMMAND,
        content: Some(format!("size/{}/{}", pty_size.rows, pty_size.cols).as_bytes().to_vec())
    };
}