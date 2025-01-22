ARG IOTA_TOOLS_IMAGE_TAG

FROM iotaledger/iota-tools:$IOTA_TOOLS_IMAGE_TAG

ARG IOTA_TOOLS_IMAGE_TAG

RUN apt update && apt -y --no-install-recommends install wget=1.21-1+deb11u1 \
        iputils-ping netcat procps bind9-host bind9-dnsutils curl iproute2 git ca-certificates awscli

# stress needs access to examples/move/basics
RUN git clone https://github.com/iotaledger/iota.git ; \
        cd iota ; \
        git checkout $IOTA_TOOLS_IMAGE_TAG ; \
        cd .. ; \
        mv iota/* .

ARG IOTA_TOOLS_IMAGE_TAG
RUN echo $IOTA_TOOLS_IMAGE_TAG > /var/run/iota_commit

COPY ./entrypoint.sh .

CMD ["./entrypoint.sh"]
