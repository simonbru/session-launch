#include <stdio.h>
#include <stdlib.h>
#include <errno.h>
#include <systemd/sd-bus.h>

const char *BUS_NAME = "simonbru.SessionLaunch";

int main(int argc, char *argv[]) {
    sd_bus *bus;
    int err;

    err = sd_bus_default_user(&bus);
    if (err < 0) {
        fprintf(stderr, "Connection error: %s\n", strerror(-err));
    }

    err = sd_bus_request_name(
        bus,
        BUS_NAME,
        SD_BUS_NAME_ALLOW_REPLACEMENT | SD_BUS_NAME_REPLACE_EXISTING
    );
    if (err < 0) {
        fprintf(stderr, "Error requesting the name: %s\n", strerror(-err));
    }

    getchar();

    sd_bus_unref(bus);
}
