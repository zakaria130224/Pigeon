# Tools for building the Windows and Linux packages from any computer that has Docker.
FROM rust:1-bookworm

ARG RUST_VERSION
RUN apt-get update \
 && apt-get install -y --no-install-recommends gcc-mingw-w64-x86-64 nsis zip \
 && if [ "$(dpkg --print-architecture)" != "amd64" ]; then \
      apt-get install -y --no-install-recommends gcc-x86-64-linux-gnu libc6-dev-amd64-cross; \
    fi \
 && rustup toolchain install "$RUST_VERSION" --profile minimal \
      --target x86_64-pc-windows-gnu --target x86_64-unknown-linux-gnu \
      --component rustfmt --component clippy

ENV CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
    AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar \
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
