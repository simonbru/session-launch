extern crate dbus;

use std::error::Error;
use std::ffi::CString;
use std::io;
use std::process::Command;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use dbus::{Connection, ConnectionItem, BusType, ErrorName, Message, NameFlag};
use dbus::tree::Factory;

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
                f.method("Exec", (), move |m| {

                    // This is the callback that will be called when another peer on the bus calls our method.
                    // the callback receives "MethodInfo" struct and can return either an error, or a list of
                    // messages to send back.

                    let (executable, args): (&str, Vec<&str>) = m.msg.read2()?;
                    let s = format!("Exec executable: {}\nArgs: {:?}", executable, args);
                    println!("{}", s);
                    let status = Command::new(&executable)
                        .args(&args)
                        .status()
                        .unwrap();

                    let status_code = match status.code() {
                        Some(code) => code,
                        None => 0
                    };
                    let mret = m.msg.method_return().append1::<i32>(status_code);


                    // Two messages will be returned - one is the method return (and should always be there),
                    // and in our case we also have a signal we want to send at the same time.
                    Ok(vec!(mret))

                // Our method has one output argument and one input argument.
                })
                .inarg::<&str,_>("executable")
                .inarg::<&str,_>("args")
            ).add_m(
                f.method("Open", (), move |m| {
                    let (executable, args): (&str, Vec<&str>) = m.msg.read2()?;
                    let s = format!("Open executable: {}\nArgs: {:?}", executable, args);
                    println!("{}", s);
                    let result = Command::new(&executable)
                        .args(&args)
                        .spawn();

                    let error_name = ErrorName::new("simonbru.SessionLaunch.Error").unwrap();
//                    let error_msg = CString::new("Error message").unwrap();

                    let mret = match result {
                        Ok(_) => m.msg.method_return(),
                        Err(ref err) if err.kind() == io::ErrorKind::NotFound => {
                            let err_name = ErrorName::new("simonbru.SessionLaunch.Error.NotFound").unwrap();
                            let err_msg = format!("{}", err);
                            let err_cstr = CString::new(err_msg).unwrap();
                            m.msg.error(&err_name, &err_cstr)
                        },
                        Err(err) => {
                            let err_str = format!("{}", err);
                            let err_cstr = CString::new(err_str).unwrap();
                            m.msg.error(&error_name, &err_cstr)
                        }
                    };

//                    let mret = m.msg.method_return();
                    Ok(vec!(mret))
                })
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