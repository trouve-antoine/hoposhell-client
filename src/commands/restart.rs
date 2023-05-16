use std::borrow::Cow;

pub fn process_restart_command(c: &Cow<str>) {
    eprintln!("Got restart command");
    std::process::exit(0);
}