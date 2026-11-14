mod args;
mod buffer_limits;
mod message;
mod run_shell;
mod connect;
mod constants;
mod credentials;
mod populate;
mod commands {
    pub mod command_error;
    /* */
    pub mod send_command_handler;
    pub mod request_or_response;
    pub mod command_processor;
    pub mod command_history;
    /* */
    pub mod restart;
    pub mod resize;
    /* */
    pub mod file_list;
    /* */
    pub mod ls;
    pub mod download;
    pub mod glob;
    pub mod http;
    pub mod tcp;
    pub mod scripts;
}
pub mod forward_tcp;

use rand::RngExt;
use rand::{self, distr::Alphanumeric};

use std::collections::HashMap;

use args::{Args, ArgsCommand};

use crate::commands::send_command_handler::main_command;

fn main() {
    buffer_limits::init();
    let args = args::parse_args();

    if args.already_connected && args.command == ArgsCommand::Connect {
        eprintln!("Got command connect but the shell is already connected");
        std::process::exit(-1);
    }

    match args.command {
        ArgsCommand::Connect => connect::main_connect(args),
        ArgsCommand::Setup => main_setup(args),
        ArgsCommand::Version => {
            eprintln!("Hoposhell Client v{}", args.version)
        },
        ArgsCommand::Command => {
            main_command(args);
        },
        ArgsCommand::Populate => {
            populate::main_populate(args);
        },
        ArgsCommand::ForwardTcp => {
            forward_tcp::main_forward_tcp(args);
        }
    }
}

fn main_setup(args: Args) {
    /* */
    match args.shell_name {
        Some(shell_name) => {
            eprintln!("Get credentials for shell {}", shell_name);
            get_shell_credentials(
                shell_name, args.api_url, 
                args.server_crt_path.unwrap(),
                args.shell_key_path.unwrap(),
                args.hoposhell_folder_path
            );
        },
        None => {
            eprintln!("Please specify the shell name");
        }
    }
}

fn get_shell_credentials(shell_name: String, api_url: String, server_crt_path: String, shell_key_path: String, hoposhell_folder_path: String) {
    eprintln!("🪙 {}/shell-credentials/request/{}", api_url, shell_name);
    reqwest::blocking::get(format!("{}/shell-credentials/request/{}", api_url, shell_name)).unwrap();
    
    let mut login_code = String::new();
    eprintln!("Enter the login code that shows on the hoposhell GUI: ");
    std::io::stdin().read_line(&mut login_code).unwrap();
    let response = reqwest::blocking::get(format!("{}/shell-credentials/confirmation/{}/{}", api_url, shell_name, login_code)).unwrap();
    let credentials = crate::buffer_limits::read(response, "credentials response").unwrap();
    let credentials = serde_json::from_slice::<HashMap<String, String>>(&credentials).unwrap();

    let server_crt = &credentials["serverCrt"];
    let shell_key = &credentials["shellKey"];

    credentials::prepare_home(std::path::Path::new(&hoposhell_folder_path))
        .expect("Unable to secure Hoposhell credentials folder");

    eprintln!("💾 Write server crt in file {}", server_crt_path);
    credentials::write(std::path::Path::new(&server_crt_path), server_crt.as_bytes())
        .expect("Unable to securely write server crt file");
    
    
    eprintln!("💾 Write shell key in file {}", shell_key_path);
    credentials::write(std::path::Path::new(&shell_key_path), shell_key.as_bytes())
        .expect("Unable to securely write shell key file");
}

fn make_random_id(n: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(n)
        .map(char::from)
        .collect::<String>()
}

enum ParseCommandResponseResult {
    CanContinue,
    ReachedLastChunk,
    Error
}
