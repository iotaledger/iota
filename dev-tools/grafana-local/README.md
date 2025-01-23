# Prometheus and Grafana monitoring for `iota-private-network`

This docker-compose configuration allows launching instances of the Prometheus and Grafana applications for monitoring of locally deployed `iota-private-network`.

In order to run this monitoring setup, you first need to have `iota-private-network` setup running, because it creates the network that Prometheus and Grafana join.

To deploy the setup, simply run `docker compose up -d`.
