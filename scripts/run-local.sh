#!/bin/sh
set -eu
: "${MODELS_DIR:?Set MODELS_DIR to an absolute persistent model directory}"
case "$MODELS_DIR" in /*) ;; *) echo 'MODELS_DIR must be absolute' >&2; exit 1;; esac
# A missing bind directory must be prepared by the operator with write access for
# UID 99/GID 100. Never chmod/chown existing model trees automatically.
set -- docker run --detach --name "${CONTAINER_NAME:-kroko-stt-wyoming-local}" \
 --platform linux/amd64 --restart=no --read-only --cap-drop=ALL \
 --security-opt=no-new-privileges --stop-timeout=25 \
 --publish "${BIND_ADDRESS:-127.0.0.1}:${HOST_PORT:-10321}:10321" \
 --mount "type=bind,src=$MODELS_DIR,dst=/models" \
 --env "MODEL_AUTO_DOWNLOAD=${MODEL_AUTO_DOWNLOAD:-true}" --env NUM_THREADS=1 --env "LANGUAGE=${LANGUAGE:-de}"
if [ -n "${KROKO_MODEL:-}" ]; then set -- "$@" --env "KROKO_MODEL=$KROKO_MODEL"; fi
if [ -n "${MODEL:-}" ]; then set -- "$@" --env "MODEL=$MODEL"; fi
if [ -n "${CUSTOM_MODEL_PATH:-}" ]; then set -- "$@" --env "CUSTOM_MODEL_PATH=$CUSTOM_MODEL_PATH"; fi
exec "$@" "${IMAGE:-kroko-stt-wyoming:local}"
