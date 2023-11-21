#!/bin/bash
podman run -it --entrypoint /bin/sh eclipse-mosquitto -c "/usr/bin/mosquitto_sub -h 192.168.1.189 -v -t \"#\""
