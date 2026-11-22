#!/bin/sh
set -eu

mkdir -p /data/caddy /config/caddy
chown -R eloforge:eloforge /data /config

exec su-exec eloforge:eloforge "$@"
