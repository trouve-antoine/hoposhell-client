use std::{
    env,
    path::Path,
    time::Duration
};

const HOPOSHELL_FOLDER_NAME: &str = ".hoposhell";

#[derive(Debug, PartialEq)]
pub enum ArgsCommand {
    CONNECT, SETUP, DOWNLOAD, UPLOAD, VERSION
}

pub struct Args {
    pub version: String,
    pub already_connected: bool,
    pub use_ssl: bool,
    pub cmd: String,
    pub auto_reconnect: bool,
    pub server_url: String,
    pub api_url: String,
    pub keep_alive: Duration,
    pub read_timeout: Duration,
    pub read_timeout_sleep: Duration,
    pub server_crt_path: Option<String>,
    pub shell_key_path: Option<String>,
    pub verify_crt: bool,
    pub command: ArgsCommand,
    pub shell_name: Option<String>,
    pub file_id: Option<String>,
    pub hoposhell_folder_path: String,
    pub default_cols: u16,
    pub default_rows: u16
}

/// setup
/// - hoposhell_client setup <shell id>
/// connect:
/// - hoposhell_client <shell id>
/// - hoposhell_client connect <shell id>
/// upload (from hoposhell shell)
/// - hoposhell_client upload <file path>
/// download (from hoposhell shell)
/// - hoposhell_client download <file id>
pub fn parse_args() -> Args {
    let cmd_args: Vec<String> = env::args().collect();

    let mut shell_name: Option<String> = None;
    let mut command = ArgsCommand::CONNECT;

    if let Ok(shell_name_) = env::var("HOPOSHELL_SHELL_ID") {
        shell_name = Some(shell_name_);
    }

    let mut file_id: Option<String> = None;

    let already_connected = match env::var("HOPOSHELL_CONNECTED") {
        Ok(_) => true,
        Err(_) => false
    };

    if cmd_args.len() > 1 {
        match cmd_args[1].as_str() {
            "connect" => {
                shell_name = Some(cmd_args[2].clone());
                command = ArgsCommand::CONNECT;
            }
            "setup" => {
                shell_name = Some(cmd_args[2].clone());
                command = ArgsCommand::SETUP;
            }
            "upload" => {
                file_id = Some(cmd_args[2].clone());
                command = ArgsCommand::UPLOAD;
            }
            "download" => {
                file_id = Some(cmd_args[2].clone());
                command = ArgsCommand::DOWNLOAD;
            }
            "version" => {
                file_id = None;
                command = ArgsCommand::VERSION;
            }
            _ => {
                shell_name = Some(cmd_args[1].clone());
                command = ArgsCommand::CONNECT;
            }
        }
    }

    let hoposhell_folder_name = env::var("HOPOSHELL_HOME_NAME").unwrap_or_else(|_| {
        String::from_utf8(HOPOSHELL_FOLDER_NAME.as_bytes().to_vec()).unwrap()
    });
    let hoposhell_folder_path = Path::new(&env::var("HOME").unwrap()).join(hoposhell_folder_name);

    let default_cols: u16 = env::var("COLS").unwrap_or_else(|_| String::from("80")).parse().unwrap();
    let default_rows: u16 = env::var("ROWS").unwrap_or_else(|_| String::from("24")).parse().unwrap();

    let mut args = Args {
        version: String::from(env!("CARGO_PKG_VERSION")),
        auto_reconnect: false,
        already_connected: already_connected,
        cmd: match env::var("SHELL") {
            Ok(x) => x,
            Err(_) => String::from("bash")
        },
        use_ssl: true,
        server_url: String::from("api.hoposhell.com:10000"),
        api_url: String::from("https://api.hoposhell.com"),
        keep_alive:Duration::from_millis(5000),
        read_timeout: Duration::from_millis(50),
        read_timeout_sleep: Duration::ZERO,
        server_crt_path: Some(String::from(hoposhell_folder_path.join("server.crt").to_str().unwrap())),
        shell_key_path: if let Some(shell_name) = shell_name.as_ref() {
            Some(format!("{}/{}.pem", hoposhell_folder_path.to_str().unwrap(), shell_name))
        } else { None },
        verify_crt: true,
        command,
        shell_name,
        file_id,
        hoposhell_folder_path: String::from(hoposhell_folder_path.to_str().unwrap()),
        default_cols,
        default_rows
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
    
    let api_url = env::var("HOPOSHELL_API");
    if let Ok(api_url) = api_url {
        args.api_url = api_url;
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

fn parse_duration_from_ms_str(time_ms_str: String) -> Duration {
    let time_ms: u64 = time_ms_str.parse().unwrap();
    return Duration::from_millis(time_ms);

}