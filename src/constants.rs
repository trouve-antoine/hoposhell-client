pub const HALF_BUF_SIZE: usize = 1024 * 8; // used in tests
pub const BUF_SIZE: usize = 1024 * 16;
pub const COMMAND_PAYLOAD_SIZE: usize = 1024 * 10; // Must be smaller than BUF_SIZE

pub const MAX_MESSAGE_HISTORY_SIZE: usize = 2048;
pub const MESSAGE_PARTS_SEPARATOR: u8 = b'/';