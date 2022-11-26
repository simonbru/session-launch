extern crate dbus;

use dbus::ffidisp::{BusType, Connection};
use dbus::message::Message;
use std::env;
use std::process::exit;

fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let con = Connection::get_private(BusType::Session)?;

    let current_dir = env::current_dir()?;
    let current_dir_str = current_dir
        .to_str()
        .ok_or("Current dir must be a valid utf8 string")?;

    let sysargs: Vec<String> = env::args().collect();
    if sysargs.len() < 2 {
        return Err(Box::from("Not enough arguments"));
    }

    let procname = &sysargs[0];
    let executable = &sysargs[1];
    let args = &sysargs[2..];

    let synchronous_exec = procname.to_lowercase().contains("exec");

    let method_name = if synchronous_exec { "Exec" } else { "Open" };
    let msg = Message::new_method_call(
        "simonbru.SessionLaunch",
        "/simonbru/SessionLaunch",
        "simonbru.SessionLaunch",
        method_name,
    )?
    .append3(current_dir_str, executable, args);

    let timeout_msec = std::i32::MAX;
    let resp = con.send_with_reply_and_block(msg, timeout_msec);
    match resp {
        Ok(ref msg) if synchronous_exec => {
            Ok(msg.get1::<i32>().ok_or("Expected an exit code in answer")?)
        }
        Ok(_) => Ok(0),
        Err(error) => {
            Err(Box::from(error))
        }
    }
}

fn main() {
    match run() {
        Ok(exit_code) => exit(exit_code),
        Err(error) => {
            eprintln!("Error: {:?}", error);
            exit(111)
        }
    };
}
