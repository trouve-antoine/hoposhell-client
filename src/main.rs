mod args;
mod message;
mod run_shell_command;
mod connect;
mod constants;
mod commands {
    pub mod request_or_response;
    pub mod command_processor;
    pub mod command_history;
    pub mod restart;
    pub mod resize;
    pub mod ls;
}

use std::{
    path::Path,
    collections::HashMap
};

use args::{Args, ArgsCommand};

fn main() {
    let args = args::parse_args();

    println!("Got command {:?}", args.command);

    if args.already_connected && args.command == ArgsCommand::CONNECT {
        eprintln!("Got command connect but the shell is already connected");
        std::process::exit(-1);
    }

    match args.command {
        ArgsCommand::CONNECT => connect::main_connect(args),
        ArgsCommand::SETUP => main_setup(args),
        ArgsCommand::DOWNLOAD => main_download(args),
        ArgsCommand::UPLOAD => main_upload(args),
        ArgsCommand::VERSION => {
            eprintln!("Hoposhell Client v{}", args.version)
        }
    }
}

fn main_setup(args: Args) {
    /* */
    match args.shell_name {
        Some(shell_name) => {
            println!("Get credentials for shell {}", shell_name);
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

fn main_download(args: Args) {
    /* */
    match args.file_id {
        Some(file_id) => {
            eprintln!("Will download file with ID {}", file_id);
        }
        None => {
            eprintln!("Please specify the file to download");
        }
    }
}

fn main_upload(args: Args) {
    /* */
    match args.file_id {
        Some(file_path_str) => {
            let file_path = Path::new(&file_path_str).canonicalize().unwrap();
            eprintln!("Will upload file at {}", file_path.to_str().unwrap());
        }
        None => {
            eprintln!("Please specify the file to upload");
        }
    }
}

fn get_shell_credentials(shell_name: String, api_url: String, server_crt_path: String, shell_key_path: String, _hoposhell_folder_path: String) {
    eprintln!("🪙 {}/shell-credentials/request/{}", api_url, shell_name);
    reqwest::blocking::get(format!("{}/shell-credentials/request/{}", api_url, shell_name)).unwrap();
    
    let mut login_code = String::new();
    println!("Enter the login code that shows on the hoposhell GUI: ");
    std::io::stdin().read_line(&mut login_code).unwrap();
    let credentials = reqwest::blocking::get(format!("{}/shell-credentials/confirmation/{}/{}", api_url, shell_name, login_code)).unwrap()
        .json::<HashMap<String, String>>().unwrap();

    let server_crt = &credentials["serverCrt"];
    let shell_key = &credentials["shellKey"];

    let server_crt_folder_path = Path::new(&server_crt_path).parent().unwrap();
    if !server_crt_folder_path.exists() {
        println!("💾 Create folder {}", server_crt_folder_path.to_str().unwrap());
        std::fs::create_dir_all(server_crt_folder_path).unwrap();
    }
    println!("💾 Write server crt in file {}", server_crt_path);
    std::fs::write(&server_crt_path, server_crt).expect("Unable to write server crt file");
    
    
    let shell_key_folder_path = Path::new(&shell_key_path).parent().unwrap();
    if !shell_key_folder_path.exists() {
        println!("💾 Create folder {}", shell_key_folder_path.to_str().unwrap());
        std::fs::create_dir_all(shell_key_folder_path).unwrap();
    }
    println!("💾 Write shell key in file {}", shell_key_path);
    std::fs::write(&shell_key_path, shell_key).expect("Unable to write shell key file");
    
    // println!("💾 Prepare hopo command {}", shell_key_path);
    // let hoposhell_folder_path = Path::new(&hoposhell_folder_path);
    // if !hoposhell_folder_path.exists() {
    //     println!("💾 Create folder {}", hoposhell_folder_path.to_str().unwrap());
    //     std::fs::create_dir_all(hoposhell_folder_path).unwrap();
    // }
    // let hoposhell_exe_path =  std::env::current_exe().unwrap();
    // std::fs::copy(hoposhell_exe_path, hoposhell_folder_path.join("hopo")).unwrap();
}

