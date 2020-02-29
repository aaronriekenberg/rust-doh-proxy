FROM rustembedded/cross:x86_64-unknown-linux-gnu-0.2.0

RUN apt-get update && \
    apt-get -y install libssl-dev
