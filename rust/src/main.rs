extern crate dbus;

use std::error::Error;
use std::ffi::CString;
use std::io;
use std::process::{Child, Command, ExitStatus};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use dbus::{Connection, ConnectionItem, BusType, ErrorName, Message, NameFlag};
use dbus::tree::{Factory, MethodInfo, MethodResult, MTSync};


fn method_error(method_info: &MethodInfo<MTSync<()>, ()>, error: &io::Error, error_name: &str) -> Message {
    let err_name = ErrorName::new(error_name).unwrap();
    let err_msg = format!("{}", error);
    let err_cstr = CString::new(err_msg).unwrap();
    method_info.msg.error(&err_name, &err_cstr)
}

fn method_exec(method_info: &MethodInfo<MTSync<()>, ()>, async: bool) -> MethodResult {
    // This is the callback that will be called when another peer on the bus calls our method.
    // the callback receives "MethodInfo" struct and can return either an error, or a list of
    // messages to send back.

    let (workdir, executable, args): (&str, &str, Vec<&str>) = method_info.msg.read3()?;
    println!("Exec {}: {}\nArgs: {:?}", if async {"async"} else {"sync"}, executable, args);

    enum CommandResult {
        Sync(ExitStatus),
        Async,
        Error(io::Error),
    }

    let mut command = Command::new(&executable);
    command.args(&args).current_dir(workdir);
    let result = if async {
        match command.spawn() {
            Ok(_) => CommandResult::Async,
            Err(e) => CommandResult::Error(e),
        }
    } else {
        match command.status() {
            Ok(status) => CommandResult::Sync(status),
            Err(e) => CommandResult::Error(e),
        }
    };

    let mret = match result {
        CommandResult::Sync(status) => {
            let status_code = match status.code() {
                Some(code) => code,
                None => 0
            };
            method_info.msg.method_return().append1::<i32>(status_code)
        },
        CommandResult::Async => {
            method_info.msg.method_return()
        },
        CommandResult::Error(ref err) if err.kind() == io::ErrorKind::NotFound => {
            method_error(method_info, err, "simonbru.SessionLaunch.Error.NotFound")
        },
        CommandResult::Error(ref err) => {
            method_error(method_info, err, "simonbru.SessionLaunch.Error.Unknown")
        }
    };
    Ok(vec!(mret))
}

fn main() {
    let wait_duration_before_exit = Duration::from_secs(30);

    // Let's start by starting up a connection to the session bus and register a name.
    let c = Connection::get_private(BusType::Session).unwrap();
    c.register_name("simonbru.SessionLaunch", NameFlag::ReplaceExisting as u32).unwrap();

    // The choice of factory tells us what type of tree we want,
    // and if we want any extra data inside. We pick the simplest variant.
    let f = Factory::new_sync::<()>();


    // We create a tree with one object path inside and make that path introspectable.
    let tree = f.tree(()).add(
        f.object_path("/simonbru/SessionLaunch", ()).introspectable().add(

            // We add an interface to the object path...
            f.interface("simonbru.SessionLaunch", ()).add_m(

                // ...and a method inside the interface.
                f.method("Exec", (), move |m| method_exec(m, false))
                .inarg::<&str,_>("workdir")
                .inarg::<&str,_>("executable")
                .inarg::<&str,_>("args")
            ).add_m(
                f.method("Open", (), move |m| method_exec(m, true))
                .inarg::<&str,_>("workdir")
                .inarg::<&str,_>("executable")
                .inarg::<&str,_>("args")
            )
        )
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
                thread_reference;
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
//        println!("{:?}, {:?}", nb_pending_requests, last_action_time.elapsed());
        if nb_pending_requests == 0
            && last_action_time.elapsed() > wait_duration_before_exit
        {
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