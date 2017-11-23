#!/usr/bin/env python3

import subprocess
import sys
from pathlib import Path

from gi.repository import Gio, GLib


BUS_NAME = 'simonbru.SessionLaunch'
OBJECT_PATH = '/simonbru/SessionLaunch'


dbus = Gio.bus_get_sync(Gio.BusType.SESSION)

executable, *exec_args = sys.argv[1:]
workdir = Path().absolute().as_posix()
params = GLib.Variant(
    '(ssas)',
    (workdir, executable, exec_args)
)
print(params)


response = dbus.call_sync(
    BUS_NAME,
    OBJECT_PATH,
    interface_name=BUS_NAME,
    method_name='Open',
    parameters=params,
    reply_type=GLib.VariantType('()'),
    flags=Gio.DBusCallFlags.NONE,
    timeout_msec=600_000,
    cancellable=None
)
print(response)
sys.exit(0)
