extern crate dbus;

use dbus::ffidisp::{BusType, Connection};
use dbus::message::Message;
use std::env;
use std::process::exit;

fn main() {
    let con = Connection::get_private(BusType::Session).unwrap();

    let current_dir = env::current_dir().unwrap();
    //    let mut sysargs= env::args();
    //    let procname = sysargs.next().unwrap();
    //    let executable = sysargs.next().unwrap();
    //    let args: Vec<String> = sysargs.collect();

    let sysargs: Vec<String> = env::args().collect();
    if sysargs.len() < 2 {
        eprintln!("ERROR: Not enough arguments");
        exit(111);
    }
    //    let [procname, executable, ...] = sysargs;
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
    )
    .unwrap()
    .append3(current_dir.to_str().unwrap(), executable, args);

    let timeout_msec = std::i32::MAX;
    let resp = con.send_with_reply_and_block(msg, timeout_msec);
    let exit_code = match resp {
        Ok(ref msg) if synchronous_exec => msg.get1::<i32>().unwrap(),
        Ok(_) => 0,
        Err(error) => {
            println!("{:?}", error);
            222
        }
    };
    exit(exit_code);
}
