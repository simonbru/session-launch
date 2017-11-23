#!/usr/bin/env python3

import os
import subprocess
import sys
from pathlib import Path

from gi.repository import Gio, GLib


BUS_NAME = 'simonbru.SessionLaunch'
OBJECT_PATH = '/simonbru/SessionLaunch'


def retrieve_interface_def():
    path = Path(__file__).absolute().parent.parent / 'interface.xml'
    return path.read_text()


def method_call_handler(
    dbus, sender, object_path, bus_name, method_name, params, invocation
):
    workdir, executable, args = params
    print(f'Run "{executable}" with args: {args}')
    spawn_flags = GLib.SpawnFlags.SEARCH_PATH
    if method_name == 'Exec':
        spawn_flags |= GLib.SpawnFlags.DO_NOT_REAP_CHILD
    pid, *_fds = GLib.spawn_async(
        argv=[executable, *args],
        working_directory=workdir,
        flags=spawn_flags,
    )
    if method_name == 'Open':
        invocation.return_value()
    elif method_name == 'Exec':
        def callback(pid, status):
            value = GLib.Variant('(i)', (status,))
            invocation.return_value(value)
        GLib.child_watch_add(GLib.PRIORITY_DEFAULT, pid, callback)


def bus_acquired_handler(dbus, _name):
    print("Bus name acquired")
    xml = retrieve_interface_def()
    node = Gio.DBusNodeInfo.new_for_xml(xml)
    interface_info = node.interfaces[0]
    object_id = dbus.register_object(
        OBJECT_PATH, interface_info, method_call_handler
    )


def name_lost_handler(dbus, _name):
    print("Bus name lost or connection closed.")
    sys.exit(1)


Gio.bus_own_name(
    Gio.BusType.SESSION,
    BUS_NAME,
    (
        Gio.BusNameOwnerFlags.ALLOW_REPLACEMENT |
        Gio.BusNameOwnerFlags.REPLACE
    ),
    bus_acquired_handler,
    None,
    name_lost_handler,
)

loop = GLib.MainLoop()
loop.run()
