pub const PATH_VAR_SEP: &str = ":";

pub const BUF_SIZE: usize = 1024 * 16;
pub const COMMAND_PAYLOAD_SIZE: usize = 1024 * 8; // Must be smaller than BUF_SIZE

pub const MAX_MESSAGE_HISTORY_SIZE: usize = 2048;
pub const MESSAGE_PARTS_SEPARATOR: u8 = b'/';

pub const WAIT_TIME_RETRY_CNX_MS: u64 = 100;

pub const SCRIPTS_FOLDER_NAME: &str = "scripts";

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum OutputFormat {
    Json,
    Text,
    Raw
}
