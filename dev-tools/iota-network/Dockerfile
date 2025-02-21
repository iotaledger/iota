ARG IOTA_TOOLS_IMAGE_TAG

FROM iotaledger/iota-tools:$IOTA_TOOLS_IMAGE_TAG AS setup

RUN apt update
RUN apt install python3 python3-pip -y

# copy configuration files to root
COPY ./new-genesis.sh /new-genesis.sh
COPY ./genesis /genesis

WORKDIR /

ARG IOTA_NODE_A_TAG
ARG IOTA_NODE_B_TAG
ENV IOTA_NODE_A_TAG=$IOTA_NODE_A_TAG
ENV IOTA_NODE_B_TAG=$IOTA_NODE_B_TAG

RUN ./new-genesis.sh
RUN echo "IOTA_NODE_A_TAG=$IOTA_NODE_A_TAG" >> /.env
RUN echo "IOTA_NODE_B_TAG=$IOTA_NODE_B_TAG" >> /.env

FROM scratch

COPY ./docker-compose-antithesis.yaml /docker-compose.yaml
COPY /genesis/overlays/* /genesis/overlays/
COPY /genesis/static/* /genesis/static/
COPY --from=setup /genesis/files/* /genesis/files/
COPY --from=setup /.env /.env

