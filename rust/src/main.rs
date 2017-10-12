extern crate dbus;

use std::process::Command;
use std::sync::{Arc, mpsc};
use dbus::{Connection, ConnectionItem, BusType, Message, NameFlag};
use dbus::tree::Factory;

fn main() {
    // Let's start by starting up a connection to the session bus and register a name.
    let c = Connection::get_private(BusType::Session).unwrap();
    c.register_name("simonbru.SessionLaunch", NameFlag::ReplaceExisting as u32).unwrap();

    // The choice of factory tells us what type of tree we want,
    // and if we want any extra data inside. We pick the simplest variant.
    let f = Factory::new_fn::<()>();


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
                    let s = format!("Executable: {}\nArgs: {:?}", executable, args);
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
            )
        )
    );

    // We register all object paths in the tree.
    tree.set_registered(&c, true).unwrap();

    println!("Service started");

    let (replies_tx, replies_rx) = mpsc::channel();
    for item in c.iter(100) {
        use ConnectionItem::*;
        println!("item received: {:?}", item);
        if let Some(m) = item.to_message() {
            let messages = tree.handle(m);
            println!("replies: {:?}", messages);
            if let Some(messages) = messages {
                replies_tx.send(messages).unwrap();
            }
        }

        while let Ok(messages) = replies_rx.try_recv() {
            for m in messages {
                c.send(m);
            }

        }
    }
}

trait ToMessage {
    fn to_message(&self) -> Option<&Message>;
}

impl ToMessage for ConnectionItem {
    fn to_message(&self) -> Option<&Message> {
        use ConnectionItem::*;
        match *self {
            MethodCall(ref m) | Signal(ref m) | MethodReturn(ref m) => Some(m),
            _ => None,
        }
    }
}