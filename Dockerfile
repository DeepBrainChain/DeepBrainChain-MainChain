FROM ubuntu:22.04

# RUN sed -i 's@//.*archive.ubuntu.com@//mirrors.ustc.edu.cn@g' /etc/apt/sources.list

RUN apt-get -y update
RUN apt-get -y install ca-certificates curl git gnupg cmake pkg-config libssl-dev git gcc build-essential clang libclang-dev gcc-12 g++-12 protobuf-compiler

# ENV RUSTUP_UPDATE_ROOT=https://mirrors.ustc.edu.cn/rust-static/rustup
# ENV RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

COPY . /DeepBrainChain-MainChain
WORKDIR /DeepBrainChain-MainChain

RUN cargo build --release
