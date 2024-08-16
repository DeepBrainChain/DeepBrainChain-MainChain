FROM docker.io/paritytech/ci-unified:latest AS builder

WORKDIR /DeepBrainChain-MainChain
COPY . .

RUN cargo fetch
RUN cargo build --locked --release

# =============

FROM phusion/baseimage:focal-1.2.0
LABEL maintainer="DeepBrainChain Developers"

#ARG USERNAME=dbc
#RUN useradd -m -u 1000 -U -s /bin/sh -d /$USERNAME $USERNAME

COPY --from=builder /DeepBrainChain-MainChain/target/release/dbc-chain /usr/local/bin

RUN /usr/local/bin/dbc-chain --version

#USER $USERNAME
#RUN mkdir /$USERNAME/data

EXPOSE 30333 9933 9944 9615
VOLUME ["/data"]
#VOLUME ["/$USERNAME/data"]

ENTRYPOINT ["/usr/local/bin/dbc-chain"]
