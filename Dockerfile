# Local CPU-only amd64 build. See docs/licensing.md before redistributing binaries.
FROM rust:1.90.0-bookworm@sha256:3914072ca0c3b8aad871db9169a651ccfce30cf58303e5d6f2db16d1d8a7e58f AS build
ENV SHERPA_ONNX_LIB_DIR=/opt/sherpa/lib
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends cmake ninja-build unzip \
 && curl --fail --location --retry 3 https://github.com/k2-fsa/sherpa-onnx/archive/refs/tags/v1.13.8.tar.gz -o /tmp/sherpa-source.tar.gz \
 && echo 'b0374cc56dbc186d442ae73d5de743bb092470b640c4c50ce7b029044c0c4fa8  /tmp/sherpa-source.tar.gz' | sha256sum --check \
 && mkdir -p /opt/sherpa-source \
 && tar -xzf /tmp/sherpa-source.tar.gz --strip-components=1 -C /opt/sherpa-source \
 && cmake -S /opt/sherpa-source -B /opt/sherpa-build -G Ninja \
      -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/opt/sherpa \
      -DBUILD_SHARED_LIBS=ON -DSHERPA_ONNX_ENABLE_TTS=OFF \
      -DSHERPA_ONNX_ENABLE_SPEAKER_DIARIZATION=OFF \
      -DSHERPA_ONNX_ENABLE_PORTAUDIO=OFF -DSHERPA_ONNX_ENABLE_WEBSOCKET=OFF \
      -DSHERPA_ONNX_ENABLE_GPU=OFF -DSHERPA_ONNX_ENABLE_PYTHON=OFF \
      -DSHERPA_ONNX_ENABLE_TESTS=OFF -DSHERPA_ONNX_ENABLE_CHECK=OFF \
      -DSHERPA_ONNX_ENABLE_BINARY=OFF -DSHERPA_ONNX_BUILD_C_API_EXAMPLES=OFF \
      -DSHERPA_ONNX_USE_PRE_INSTALLED_ONNXRUNTIME_IF_AVAILABLE=OFF \
 && cmake --build /opt/sherpa-build --parallel 2 \
 && cmake --install /opt/sherpa-build
COPY Cargo.toml Cargo.lock ./
COPY src/server ./src/server
COPY src/bin/wyoming-probe.rs ./src/bin/wyoming-probe.rs
COPY src/bin/model-manager.rs ./src/bin/model-manager.rs
RUN cargo test --locked && cargo build --release --locked --bins
COPY scripts/collect-licenses.sh /opt/collect-licenses.sh
RUN sh /opt/collect-licenses.sh \
 && mkdir -p /opt/runtime-libs \
 && find /opt/sherpa/lib -maxdepth 1 -name '*.so*' ! -name '*cxx-api*' -exec cp -P '{}' /opt/runtime-libs/ \;
FROM gcr.io/distroless/cc-debian12@sha256:777e96cf322c46bc32aca926c263624c4dc8d7cf37e2fa65ba2c7e697318ebbb
ARG VCS_REF=uncommitted
LABEL org.opencontainers.image.source="https://github.com/marco-taylor/kroko-stt-german-wyoming" \
      org.opencontainers.image.title="Kroko STT Wyoming" \
      org.opencontainers.image.version="0.3.0" \
      org.opencontainers.image.revision=$VCS_REF
COPY --from=build /build/target/release/kroko-wyoming /usr/local/bin/kroko-wyoming
COPY --from=build /build/target/release/model-manager /usr/local/bin/model-manager
COPY --from=build /build/target/release/wyoming-probe /usr/local/bin/wyoming-probe
COPY --from=build /opt/runtime-libs/ /usr/local/lib/kroko/
COPY --from=build /opt/distribution-licenses/ /usr/share/licenses/kroko/dependencies/
COPY --from=build /usr/share/zoneinfo/Europe/Berlin /usr/share/zoneinfo/Europe/Berlin
COPY LICENSE NOTICE /usr/share/licenses/kroko/
COPY docs/licensing.md docs/dependency-licenses.md /usr/share/licenses/kroko/
USER 99:100
ENV LD_LIBRARY_PATH=/usr/local/lib/kroko MODEL_DIR=/models MODEL_AUTO_DOWNLOAD=true HOST=0.0.0.0 PORT=10321 NUM_THREADS=1 LANGUAGE=de LOG_LEVEL=info TZ=Europe/Berlin
EXPOSE 10321
HEALTHCHECK --interval=15s --timeout=5s --start-period=30m --retries=3 CMD ["/usr/local/bin/wyoming-probe"]
ENTRYPOINT ["/usr/local/bin/kroko-wyoming"]
