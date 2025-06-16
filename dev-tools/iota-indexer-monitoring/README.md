# Prometheus and Grafana monitoring for `iota-indexer`

This docker-compose configuration allows launching instances of the Prometheus and Grafana applications for monitoring of the deployed `iota-indexer` instance.

## Prerequisites

In order to run this monitoring setup, you first need to have `iota-indexer` setup running. By default, Prometheus will listen at the `client-metrics-port` at `9184` in order to scrape the metrics.
You can change this port in the `prometheus.yml` file if needed. For the indexer to correctly expose its metrics it should be in the same docker network as the prometheus and grafana services.

To deploy the setup, simply run `docker compose up -d`.

## Accessing Grafana

You can access Grafana at `http://localhost:3000` with the default credentials `admin:admin`. You can change the password the first time you log in.
The required datasource is already configured to point to Prometheus at `http://prometheus:9090` which is the default address of the Prometheus service configured in the docker-compose file.
Furthermore, the Grafana dashboard is automatically imported and can be found in the `Dashboards` section of Grafana.
