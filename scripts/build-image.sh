#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
docker build --platform linux/amd64 --tag "${IMAGE:-kroko-stt-wyoming:local}" .
