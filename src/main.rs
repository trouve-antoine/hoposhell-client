mod args;
mod message;
mod run_shell;
mod connect;
mod constants;
mod populate;
mod commands {
    pub mod command_error;
    /* */
    pub mod send_command_hanlder;
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
}

use rand::Rng;
use rand::{self, distributions::Alphanumeric};

use std::{
    path::Path,
    collections::HashMap
};

use args::{Args, ArgsCommand};

use crate::commands::send_command_hanlder::main_command;

fn main() {
    let args = args::parse_args();

    if args.already_connected && args.command == ArgsCommand::CONNECT {
        eprintln!("Got command connect but the shell is already connected");
        std::process::exit(-1);
    }

    match args.command {
        ArgsCommand::CONNECT => connect::main_connect(args),
        ArgsCommand::SETUP => main_setup(args),
        ArgsCommand::VERSION => {
            eprintln!("Hoposhell Client v{}", args.version)
        },
        ArgsCommand::COMMAND => {
            main_command(args);
        },
        ArgsCommand::POPULATE => {
            populate::main_populate(args);
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

fn get_shell_credentials(shell_name: String, api_url: String, server_crt_path: String, shell_key_path: String, _hoposhell_folder_path: String) {
    eprintln!("🪙 {}/shell-credentials/request/{}", api_url, shell_name);
    reqwest::blocking::get(format!("{}/shell-credentials/request/{}", api_url, shell_name)).unwrap();
    
    let mut login_code = String::new();
    eprintln!("Enter the login code that shows on the hoposhell GUI: ");
    std::io::stdin().read_line(&mut login_code).unwrap();
    let credentials = reqwest::blocking::get(format!("{}/shell-credentials/confirmation/{}/{}", api_url, shell_name, login_code)).unwrap()
        .json::<HashMap<String, String>>().unwrap();

    let server_crt = &credentials["serverCrt"];
    let shell_key = &credentials["shellKey"];

    let server_crt_folder_path = Path::new(&server_crt_path).parent().unwrap();
    if !server_crt_folder_path.exists() {
        eprintln!("💾 Create folder {}", server_crt_folder_path.to_str().unwrap());
        std::fs::create_dir_all(server_crt_folder_path).unwrap();
    }
    eprintln!("💾 Write server crt in file {}", server_crt_path);
    std::fs::write(&server_crt_path, server_crt).expect("Unable to write server crt file");
    
    
    let shell_key_folder_path = Path::new(&shell_key_path).parent().unwrap();
    if !shell_key_folder_path.exists() {
        eprintln!("💾 Create folder {}", shell_key_folder_path.to_str().unwrap());
        std::fs::create_dir_all(shell_key_folder_path).unwrap();
    }
    eprintln!("💾 Write shell key in file {}", shell_key_path);
    std::fs::write(&shell_key_path, shell_key).expect("Unable to write shell key file");
}

fn make_random_id(n: usize) -> String {
    rand::thread_rng()
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