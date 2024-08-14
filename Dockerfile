FROM docker.io/paritytech/ci-unified:latest AS builder

WORKDIR /DeepBrainChain-MainChain
COPY . .

RUN cargo fetch
RUN cargo build --locked --release

# =============

FROM docker.io/parity/base-bin:latest
LABEL maintainer="DeepBrainChain Developers"

COPY --from=builder /DeepBrainChain-MainChain/target/release/dbc-chain /usr/local/bin

USER root
ARG USERNAME=dbc
RUN useradd -m -u 1001 -U -s /bin/sh -d /$USERNAME $USERNAME && \
	mkdir -p /$USERNAME/data /$USERNAME/.local/share && \
	chown -R $USERNAME:$USERNAME /$USERNAME/data && \
	ln -s /$USERNAME/data /$USERNAME/.local/share/dbc-chain && \
# unclutter and minimize the attack surface
	# rm -rf /usr/bin /usr/sbin && \
# check if executable works in this container
	/usr/local/bin/dbc-chain --version

USER $USERNAME

EXPOSE 30333 9933 9944 9615
VOLUME ["/$USERNAME/data"]

ENTRYPOINT ["/usr/local/bin/dbc-chain"]
