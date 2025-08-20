use std::ffi::CString;
use std::fs::File;
use std::io;
use std::process::Command;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use dbus::ffidisp::{BusType, Connection, ConnectionItem, NameFlag};
use dbus::message::Message;
use dbus::strings::ErrorName;
use dbus_tree::{Factory, MTSync, MethodInfo, MethodResult};

fn method_error(method_info: &MethodInfo<MTSync<()>, ()>, error: &io::Error) -> Message {
    let err_name = match error.kind() {
        io::ErrorKind::NotFound => "simonbru.SessionLaunch.Error.NotFound",
        _ => "simonbru.SessionLaunch.Error.Unknown",
    };
    let err_name = ErrorName::new(err_name).unwrap();
    let err_cstr = CString::new(error.to_string()).unwrap();
    method_info.msg.error(&err_name, &err_cstr)
}

fn method_open(method_info: &MethodInfo<MTSync<()>, ()>) -> MethodResult {
    // This is the callback that will be called when another peer on the bus calls our method.
    // the callback receives "MethodInfo" struct and can return either an error, or a list of
    // messages to send back.

    let mut items = method_info.msg.iter_init();
    let workdir: &str = items.read()?;
    let executable: &str = items.read()?;
    let args: Vec<&str> = items.read()?;

    let mut command = Command::new(&executable);
    command.args(&args).current_dir(workdir);

    let return_msg = match command.spawn() {
        Ok(mut child) => {
            thread::spawn(move || {
                // Make sure that we reap zombie processes while the service is running.
                // If the service exits before all childs have exited, the init process
                // will reap child process for us.
                child.wait().unwrap();
            });
            method_info.msg.method_return()
        }
        Err(error) => method_error(method_info, &error)
    };
    Ok(vec![return_msg])
}

fn method_exec(method_info: &MethodInfo<MTSync<()>, ()>) -> MethodResult {
    let mut items = method_info.msg.iter_init();
    let workdir: &str = items.read()?;
    let executable: &str = items.read()?;
    let args: Vec<&str> = items.read()?;
    let stdin: File = items.read()?;
    let stdout: File = items.read()?;
    let stderr: File = items.read()?;

    let mut command = Command::new(&executable);
    command
        .args(&args)
        .current_dir(workdir)
        .stdin(stdin)
        .stdout(stdout)
        .stderr(stderr);

    let return_msg = match command.status() {
        Ok(status) => {
            let status_code = status.code().unwrap_or(1);
            method_info.msg.method_return().append1::<i32>(status_code)
        },
        Err(error) => method_error(method_info, &error)
    };
    Ok(vec![return_msg])
}

fn main() {
    let wait_duration_before_exit = Duration::from_secs(30);

    // Let's start by starting up a connection to the session bus and register a name.
    let c = Connection::get_private(BusType::Session).unwrap();
    c.register_name("simonbru.SessionLaunch", NameFlag::ReplaceExisting as u32)
        .unwrap();

    // The choice of factory tells us what type of tree we want,
    // and if we want any extra data inside. We pick the simplest variant.
    let f = Factory::new_sync::<()>();

    // We create a tree with one object path inside and make that path introspectable.
    let tree = f.tree(()).add(
        f.object_path("/simonbru/SessionLaunch", ())
            .introspectable()
            .add(
                // We add an interface to the object path...
                f.interface("simonbru.SessionLaunch", ())
                    .add_m(
                        // ...and a method inside the interface.
                        f.method("Exec", (), method_exec)
                            .inarg::<&str, _>("workdir")
                            .inarg::<&str, _>("executable")
                            .inarg::<&[&str], _>("args")
                            .inarg::<File, _>("stdin")
                            .inarg::<File, _>("stdout")
                            .inarg::<File, _>("stderr")
                            .outarg::<&i32, _>("status"),
                    )
                    .add_m(
                        f.method("Open", (), method_open)
                            .inarg::<&str, _>("workdir")
                            .inarg::<&str, _>("executable")
                            .inarg::<&[&str], _>("args"),
                    ),
            ),
    );

    // We register all object paths in the tree.
    tree.set_registered(&c, true).unwrap();

    println!("Service started");

    let (replies_tx, replies_rx) = mpsc::channel();
    let tree = Arc::new(tree);
    let thread_counter = Arc::new(());
    let mut last_action_time = Instant::now();
    for item in c.iter(100) {
        if let Some(msg) = item.into_message() {
            println!("item received: {:?}", msg);
            let (replies_tx, tree) = (replies_tx.clone(), tree.clone());
            let thread_reference = thread_counter.clone();
            thread::spawn(move || {
                let messages = tree.handle(&msg);
                println!("replies: {:?}", messages);
                if let Some(messages) = messages {
                    replies_tx.send(messages).unwrap();
                }
                // hack to keep ref to thread_counter until end of thread
                drop(thread_reference);
            });
            last_action_time = Instant::now();
        }

        while let Ok(messages) = replies_rx.try_recv() {
            for m in messages {
                c.send(m).unwrap();
            }
            last_action_time = Instant::now();
        }

        let nb_pending_requests = Arc::strong_count(&thread_counter) - 1;
        // println!("{:?}, {:?}", nb_pending_requests, last_action_time.elapsed());
        if nb_pending_requests == 0 && last_action_time.elapsed() > wait_duration_before_exit {
            println!(
                "Inactive for {} seconds, exiting.",
                wait_duration_before_exit.as_secs()
            );
            break;
        }
    }
}

trait ToMessage {
    fn into_message(self) -> Option<Message>;
}

impl ToMessage for ConnectionItem {
    fn into_message(self) -> Option<Message> {
        use ConnectionItem::*;
        match self {
            MethodCall(m) | Signal(m) | MethodReturn(m) => Some(m),
            _ => None,
        }
    }
}
