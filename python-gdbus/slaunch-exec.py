#!/usr/bin/env python3

import subprocess
import sys
from pathlib import Path

from gi.repository import Gio, GLib


BUS_NAME = 'simonbru.SessionLaunch'
OBJECT_PATH = '/simonbru/SessionLaunch'


dbus = Gio.bus_get_sync(Gio.BusType.SESSION)

params = GLib.Variant(
    '(sas)',
    (sys.argv[1], sys.argv[2:])
)
print(params)


status, = dbus.call_sync(
    BUS_NAME,
    OBJECT_PATH,
    interface_name=BUS_NAME,
    method_name='Exec',
    parameters=params,
    reply_type=GLib.VariantType('(i)'),
    flags=Gio.DBusCallFlags.NONE,
    timeout_msec=600_000,
    cancellable=None
)
print(status)
sys.exit(status)