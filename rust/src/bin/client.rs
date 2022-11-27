use std::env;
use std::io::BufReader;
use std::os::unix::io::AsFd;
use std::process::exit;

use dbus::ffidisp::{BusType, Connection};
use dbus::message::Message;

use session_launch::run_stream_pipe;

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
    let mut msg = Message::new_method_call(
        "simonbru.SessionLaunch",
        "/simonbru/SessionLaunch",
        "simonbru.SessionLaunch",
        method_name,
    )?
    .append3(current_dir_str, executable, args);

    if synchronous_exec {
        // Create pipes that forward standard input/output.
        // Make sure that unecessary pipe references are dropped ASAP
        let (stdin_reader, stdin_writer) = os_pipe::pipe()?;
        let stdin_reader_file = std::fs::File::from(stdin_reader.as_fd().try_clone_to_owned()?);

        let (stdout_reader, stdout_writer) = os_pipe::pipe()?;
        let stdout_writer_file = std::fs::File::from(stdout_writer.as_fd().try_clone_to_owned()?);

        let (stderr_reader, stderr_writer) = os_pipe::pipe()?;
        let stderr_writer_file = std::fs::File::from(stderr_writer.as_fd().try_clone_to_owned()?);

        msg = msg.append3(stdin_reader_file, stdout_writer_file, stderr_writer_file);

        std::thread::spawn(move || {
            let stdin = std::io::stdin().lock();
            run_stream_pipe(Box::new(stdin), Box::new(stdin_writer));
        });
        std::thread::spawn(move || {
            run_stream_pipe(
                Box::new(BufReader::new(stdout_reader)),
                Box::new(std::io::stdout()),
            );
        });
        std::thread::spawn(move || {
            run_stream_pipe(
                Box::new(BufReader::new(stderr_reader)),
                Box::new(std::io::stderr()),
            );
        });
    };

    let timeout_msec = std::i32::MAX;
    let resp = con.send_with_reply_and_block(msg, timeout_msec);
    match resp {
        Ok(ref msg) if synchronous_exec => {
            Ok(msg.get1::<i32>().ok_or("Expected an exit code in answer")?)
        }
        Ok(_) => Ok(0),
        Err(error) => Err(Box::from(error)),
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
