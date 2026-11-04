# Development and local checks

The public application consists of the server modules under `src/server/` and
`src/bin/wyoming-probe.rs` and `src/bin/model-manager.rs`. There are three explicitly declared Cargo binaries;
historical local benchmark code is excluded and automatic example discovery is
disabled. Dependencies remain pinned by Cargo.lock. No additional runtime was added.

The Docker build runs `cargo test --locked` and `cargo build --release --locked
--bins` using a source-built native sherpa C API with TTS and diarization disabled.
The official Rust `shared` feature is used; there is no custom FFI or second
inference runtime. Fifteen distinct unit tests cover protocol framing/limits, model selection,
local-first validation, safe staging, failed downloads and scoped removal. Shared
modules execute these tests across all three binaries (30 test executions). The probe additionally verifies Describe and
can send existing WAV audio in paced 20 ms chunks with intentionally fragmented
TCP writes. No regression audio is distributed because its redistribution
provenance has not been established.

The probe's smoke mode checks only the two named fixtures and their expected
words: `01.wav` ("Schalte das Licht im Wohnzimmer ein.") and `03.wav`
("Stelle die Temperatur im Wohnzimmer auf zweiundzwanzig Grad."). It is not an
arbitrary WAV transcription tool. Provide corresponding 16 kHz mono PCM16
recordings separately, mount their directory read-only at `/test` in a separate
test container, then run:

```sh
docker exec YOUR_TEST_CONTAINER /usr/local/bin/wyoming-probe --smoke 127.0.0.1:10321 /test/01.wav
```

Wait for Docker's healthy state before probing: startup includes file hashing and
model loading. `--smoke-fast` performs an unpaced throughput check. File names
`01.wav` and `03.wav` have built-in expected command checks; other filenames are
rejected. This probe restriction does not restrict the server to those phrases.

Public source preparation was verified with two consecutive requests, model
loading once, partial results before end-of-input, and graceful SIGTERM shutdown.
This check does not replace the previous real Home Assistant test.

Local model weights, audio, results, research, caches, vendored snapshots and
historical scripts are intentionally ignored. Do not use `git add -f` on them.
No automatic publishing workflow or credentials are distributed. Building locally
does not publish a release or upload to a registry.

The server entrypoint is `src/server/main.rs`, producing `kroko-wyoming`.
`src/main.rs` and historical examples are not Cargo targets. The probe reuses
the protocol module, including server-only helpers. Dead-code warnings are
suppressed for that module in the client and for the shared model-manager module
whose CLI-only/server-only members are intentionally unused in the other binary.
Server inference/protocol lint checks remain active.

With Rust 1.90.0 and matching native libraries available, run:

```sh
SHERPA_ONNX_LIB_DIR=/absolute/path/to/native/lib cargo test --locked
SHERPA_ONNX_LIB_DIR=/absolute/path/to/native/lib cargo build --release --locked --bins
```

Use the TTS-disabled shared libraries built by the Dockerfile, rather than the
generic upstream TTS-enabled prebuilt archive. The library directory is needed for both commands. A fresh dependency cache
requires internet access; an existing matching cache supports offline builds.

## Model-manager acceptance test

With a local candidate image and independently supplied local model/audio fixtures:

```sh
python3 tests/model-manager-smoke.py \
  --image kroko-stt-german-wyoming:v0.2.0-test \
  --fixtures /absolute/path/to/local/models \
  --wav-dir /absolute/path/to/local/neural-wavs \
  --output /absolute/path/to/a/new/test-output-directory \
  --download
```

`--download` explicitly enables the real pinned provider test (~156 MB) and a
short interrupted-download check. Without it the network acceptance cases are
skipped. The script creates only uniquely named test containers, publishes no
host ports, uses internal port 19321, UID 99/GID 100 and a read-only container
rootfs. Local fixtures are copied into new test directories, never modified.
Existing licensed fixture models `classic` and `community-128` and the two WAVs
are prerequisites; no models or audio are distributed with these tests.
Python is a development-test dependency, not an image/runtime dependency.
Release validation and limitations: [v0.2.0 release notes](releases/v0.2.0.md).
