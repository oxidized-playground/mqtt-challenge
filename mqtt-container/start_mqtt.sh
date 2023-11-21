#!/bin/bash
podman run -it --privileged -p 1883:1883 -v ./mosquitto.conf:/mosquitto/config/mosquitto.conf eclipse-mosquitto
