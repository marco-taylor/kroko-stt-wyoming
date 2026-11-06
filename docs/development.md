# Development and local checks

The public application consists of the server modules under `src/server/` and
`src/bin/wyoming-probe.rs` and `src/bin/model-manager.rs`. There are three explicitly declared Cargo binaries;
historical local benchmark code is excluded and automatic example discovery is
disabled. Dependencies remain pinned by Cargo.lock. No additional inference runtime was added. The package and new container name are `kroko-stt-wyoming`. Canonical GitHub and GHCR names are `kroko-stt-wyoming` from v0.3.1. Existing saved container/model mounts remain unchanged.

The Docker build runs `cargo test --locked` and `cargo build --release --locked
--bins` using a source-built native sherpa C API with TTS and diarization disabled.
The official Rust `shared` feature is used; there is no custom FFI or second
inference runtime. The release build runs all unit tests across the three binaries. Shared
modules execute some tests in multiple binaries. Coverage includes framing,
language/model identity, local-first validation, safe staging, selected-only
downloads, package cleanup guards and scoped removal. The probe additionally verifies Describe and
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
  --image kroko-stt-wyoming:release-test \
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

## Local custom-package regression checks

The development candidate adds explicit local free `.data` selection. With existing official
Community packages and a separately supplied German PCM16/16 kHz mono fixture:

```sh
python3 tests/custom-data-smoke.py \
  --image kroko-stt-wyoming:release-test \
  --models /absolute/path/to/local/models \
  --audio /absolute/path/to/01.wav \
  --output /absolute/path/to/a/new/custom-test-output
```

The expected sentence is "Schalte das Licht im Wohnzimmer ein". This test
uses uniquely named isolated containers, random loopback host ports and copied
model packages. It checks explicit selection with two packages, preparation,
Wyoming through the host port, restart without preparation, offline/read-only
startup and transcription, UID/GID 99:100, scoped errors and preservation of
invalid caches. A synthetic `free=false` header verifies rejection before
writes; no protected model is needed. No downloads or
publications occur. Test-output model copies can be removed after all test
containers have been stopped; never use production storage as the output path.

## Multilingual checks

The catalogue is embedded from `src/server/model_catalog.json`. Language and
variant form the identity; German v0.2.0 paths are legacy aliases only for `de`.
`tests/multilingual-smoke.py` verifies real German variant switching, package
cleanup after preparation, failure
without fallback when English is absent. Optional already authorized English
fixtures enable native English tests. It never downloads packages itself.
The existing smoke scripts also accept `--binaries /path/to/new/debug-or-release`
to mount newly compiled binaries into an isolated existing local runtime image.
Do not confuse this with a newly published image or a performance benchmark.

## Unraid template contract

Run `python3 tests/template-test.py` from the checkout. It checks the canonical
names and links, the language catalogue, the single persistent model mount, host
port mapping, exactly one app XML with a matching TemplateURL, the repository
profile, and absence of internal HOST/PORT/MODEL_DIR
options. Runtime defaults remain in the Dockerfile and server configuration.
