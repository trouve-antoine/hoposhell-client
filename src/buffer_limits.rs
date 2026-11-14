use std::{
    env,
    io::{self, Read, Write},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, OnceLock,
    },
};

const MIB: usize = 1024 * 1024;
const DEFAULT_MB: usize = 64;
const ENV_NAME: &str = "HOPOSHELL_MAX_BUFFER_MB";
static LIMIT: OnceLock<usize> = OnceLock::new();
static QUEUE_BUDGET: OnceLock<Arc<Budget>> = OnceLock::new();

fn parse_limit(value: &str) -> Result<usize, &'static str> {
    value
        .parse::<usize>()
        .ok()
        .filter(|mb| *mb > 0)
        .and_then(|mb| mb.checked_mul(MIB))
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or("must be a positive whole number of MiB within the platform's size limit")
}

pub fn init() {
    limit();
}

pub fn limit() -> usize {
    *LIMIT.get_or_init(|| match env::var(ENV_NAME) {
        Ok(value) => parse_limit(&value).unwrap_or_else(|error| {
            eprintln!("{ENV_NAME}: {error}");
            std::process::exit(1);
        }),
        Err(env::VarError::NotPresent) => DEFAULT_MB * MIB,
        Err(error) => {
            eprintln!("{ENV_NAME}: {error}");
            std::process::exit(1);
        }
    })
}

fn exceeded(context: &str) -> ! {
    eprintln!(
        "Buffer limit exceeded ({context}); {ENV_NAME}={} MiB. Exiting.",
        limit() / MIB
    );
    // A panic in a worker thread would leave the rest of the client alive.
    std::process::exit(1);
}

pub fn check(size: usize, context: &str) {
    if size > limit() {
        exceeded(context);
    }
}

pub fn growth(current: usize, additional: usize, context: &str) -> usize {
    let total = current
        .checked_add(additional)
        .unwrap_or_else(|| exceeded(context));
    check(total, context);
    total
}

pub fn append(buffer: &mut Vec<u8>, bytes: &[u8], context: &str) {
    growth(buffer.len(), bytes.len(), context);
    // Avoid geometric capacity growth beyond the configured buffer limit.
    buffer.reserve_exact(bytes.len());
    buffer.extend_from_slice(bytes);
}

pub fn read(mut reader: impl Read, context: &str) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut chunk = [0; 16 * 1024];
    loop {
        let count = match reader.read(&mut chunk) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(output);
        }
        append(&mut output, &chunk[..count], context);
    }
}

pub struct Writer {
    bytes: Vec<u8>,
    context: &'static str,
}

impl Writer {
    pub fn new(context: &'static str) -> Self {
        Self {
            bytes: Vec::new(),
            context,
        }
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        append(&mut self.bytes, bytes, self.context);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Budget {
    used: AtomicUsize,
    max: usize,
}

impl Budget {
    fn reserve(self: &Arc<Self>, size: usize) -> Option<Reservation> {
        let mut used = self.used.load(Ordering::Relaxed);
        loop {
            let next = used.checked_add(size).filter(|total| *total <= self.max)?;
            match self
                .used
                .compare_exchange_weak(used, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => {
                    return Some(Reservation {
                        budget: self.clone(),
                        size,
                    })
                }
                Err(current) => used = current,
            }
        }
    }
}

struct Reservation {
    budget: Arc<Budget>,
    size: usize,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.budget.used.fetch_sub(self.size, Ordering::Relaxed);
    }
}

pub trait BufferedSize {
    fn buffered_size(&self) -> usize;
}

pub struct Sender<T>(mpsc::Sender<(T, Reservation)>, Arc<Budget>);
pub struct Receiver<T>(mpsc::Receiver<(T, Reservation)>);

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let budget = QUEUE_BUDGET
        .get_or_init(|| {
            Arc::new(Budget {
                used: AtomicUsize::new(0),
                max: limit(),
            })
        })
        .clone();
    let (sender, receiver) = mpsc::channel();
    (Sender(sender, budget), Receiver(receiver))
}

impl<T: BufferedSize> Sender<T> {
    pub fn send(&self, message: T) -> Result<(), mpsc::SendError<T>> {
        let size = message
            .buffered_size()
            .checked_add(std::mem::size_of::<Reservation>())
            .unwrap_or_else(|| exceeded("message queues"));
        let reservation = self
            .1
            .reserve(size)
            .unwrap_or_else(|| exceeded("message queues"));
        self.0
            .send((message, reservation))
            .map_err(|error| mpsc::SendError(error.0 .0))
    }
}

impl<T> Receiver<T> {
    pub fn recv(&self) -> Result<T, mpsc::RecvError> {
        self.0.recv().map(|(message, _reservation)| message)
    }

    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Result<T, mpsc::RecvTimeoutError> {
        self.0
            .recv_timeout(timeout)
            .map(|(message, _reservation)| message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_positive_mebibytes_only() {
        assert_eq!(parse_limit("1"), Ok(MIB));
        assert_eq!(parse_limit("64"), Ok(64 * MIB));
        for value in ["", "0", "-1", "1.5", "abc", "18446744073709551615"] {
            assert!(parse_limit(value).is_err(), "{value}");
        }
    }

    #[test]
    fn queue_budget_is_shared_and_released() {
        let budget = Arc::new(Budget {
            used: AtomicUsize::new(0),
            max: 10,
        });
        let first = budget.reserve(6).unwrap();
        assert!(budget.reserve(5).is_none());
        let second = budget.reserve(4).unwrap();
        assert!(budget.reserve(1).is_none());
        drop(first);
        assert!(budget.reserve(6).is_some());
        drop(second);
        assert_eq!(budget.used.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn reads_and_writes_small_buffers() {
        assert_eq!(read(&b"hello"[..], "test").unwrap(), b"hello");
        let mut writer = Writer::new("test");
        writer.write_all(b"hello").unwrap();
        assert_eq!(writer.into_inner(), b"hello");
    }

    #[test]
    fn limit_exit_probe() {
        if let Ok(probe) = env::var("HOPOSHELL_BUFFER_TEST_PROBE") {
            match probe.as_str() {
                "read" => {
                    let _ = read(io::repeat(0).take((MIB + 1) as u64), "test output");
                }
                "writer" => {
                    let mut writer = Writer::new("test writer");
                    for _ in 0..65 {
                        writer.write_all(&[0; 16 * 1024]).unwrap();
                    }
                }
                "queues" => {
                    use crate::message::{Message, MessageTypeToStream};
                    let (first, _first_receiver) = channel();
                    let (second, _second_receiver) = channel();
                    for _ in 0..33 {
                        for sender in [&first, &second] {
                            sender
                                .send(Message {
                                    mtype: MessageTypeToStream::STDOUT,
                                    content: Some(vec![0; 16 * 1024]),
                                })
                                .unwrap();
                        }
                    }
                }
                "frame" => {
                    let mut frame = String::new();
                    let data = [b'a'; crate::constants::BUF_SIZE];
                    for _ in 0..65 {
                        crate::message::separate_messages(&mut frame, &data, data.len());
                    }
                }
                "pending" => {
                    use crate::commands::request_or_response::{ChunkType, ChunkedRequest};
                    let mut history = crate::commands::command_history::CommandHistory::new();
                    for id in 0..300 {
                        let request = ChunkedRequest {
                            creation_timestamp: 0,
                            cmd: "download".into(),
                            message_id: id.to_string(),
                            target: "shell:test".into(),
                            chunk_type: ChunkType::NotLast,
                            payload: vec![0; 4096],
                        };
                        history.append(&request.to_message_payload());
                    }
                }
                "overflow" => {
                    growth(usize::MAX, 1, "test overflow");
                }
                _ => panic!("unknown test probe"),
            }
            panic!("oversized buffer did not exit");
        }
    }

    #[test]
    fn exceeding_limit_exits_the_process() {
        for probe in ["read", "writer", "queues", "frame", "pending", "overflow"] {
            let output = std::process::Command::new(env::current_exe().unwrap())
                .args([
                    "--exact",
                    "buffer_limits::tests::limit_exit_probe",
                    "--nocapture",
                ])
                .env("HOPOSHELL_BUFFER_TEST_PROBE", probe)
                .env(ENV_NAME, "1")
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1), "{probe}: {:?}", output);
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("Buffer limit exceeded"),
                "{probe}"
            );
        }
    }

    #[test]
    fn accepts_exact_limit() {
        // Use a child process so environment-dependent OnceLock state is isolated.
        let output = std::process::Command::new(env::current_exe().unwrap())
            .args(["--exact", "buffer_limits::tests::exact_limit_probe"])
            .env("HOPOSHELL_BUFFER_TEST_EXACT", "1")
            .env(ENV_NAME, "1")
            .output()
            .unwrap();
        assert!(output.status.success());
    }

    #[test]
    fn exact_limit_probe() {
        if env::var_os("HOPOSHELL_BUFFER_TEST_EXACT").is_some() {
            assert_eq!(
                read(io::repeat(0).take(MIB as u64), "test").unwrap().len(),
                MIB
            );
        }
    }
}
