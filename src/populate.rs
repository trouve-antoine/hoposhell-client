use std::{path::Path, os::unix::prelude::PermissionsExt};

use crate::args::Args;

const IMGCAT_URL: &str = "https://iterm2.com/utilities/imgcat";

pub fn main_populate(args: Args) {
    let bin_folder_path = Path::new(&args.hoposhell_folder_path).join("bin");
    let bin_folder_path = bin_folder_path.to_str().unwrap();
    let bin_folder_path = bin_folder_path.to_string();

    eprintln!("📂 Create bin folder: {}", bin_folder_path);
    std::fs::create_dir_all(&bin_folder_path).unwrap();

    let imgcat_path = Path::new(&bin_folder_path).join("imgcat");
    let imgcat_contents = reqwest::blocking::get(IMGCAT_URL).unwrap().text().unwrap();

    eprintln!("💾 Download imgcat: {} -> {}", IMGCAT_URL, imgcat_path.to_str().unwrap());
    std::fs::write(&imgcat_path, imgcat_contents).unwrap();
    let mut perms = std::fs::metadata(&imgcat_path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&imgcat_path, perms).unwrap();
}