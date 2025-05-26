# IOTA-Names indexer

Indexer to collect metrics about IOTA-Names.

## Testing

### Run a local network

```bash
iota start --force-regenesis --with-faucet --committee-size 2
```

### Deploy the IOTA-Names packages

https://github.com/iotaledger/iota-names/tree/develop/scripts#setup-iota-names-locally
In the `iota-names` directory, run:

```bash
pnpm ts-node init/init.ts localnet
```

### Start the indexer

```bash
cargo run
```

### Start prometheus

```bash
# Create persistent volume for your data
docker volume create prometheus-data
# Start Prometheus container
docker run \
    -p 9090:9090 \
    -v "$(pwd)/prometheus.yml:/etc/prometheus/prometheus.yml" \
    -v prometheus-data:/prometheus \
    prom/prometheus
```

### Start Grafana

```bash
sudo /bin/systemctl start grafana-server
```

Set env variables:

JSON_FILE="./localnet.json"

export IOTA_NAMES_PACKAGE_ADDRESS=$(jq -r '.packageId' "$JSON_FILE")
export IOTA_NAMES_OBJECT_ID=$(jq -r '.iotaNames' "$JSON_FILE")
export IOTA_NAMES_PAYMENTS_PACKAGE_ADDRESS=$(jq -r '.paymentsPackageId' "$JSON_FILE")
export IOTA_NAMES_REGISTRY_ID=$(jq -r '.registryTableId' "$JSON_FILE")
export IOTA_NAMES_REVERSE_REGISTRY_ID=$(jq -r '.reverseRegistryTableId' "$JSON_FILE")

echo "IOTA_NAMES_PACKAGE_ADDRESS=$IOTA_NAMES_PACKAGE_ADDRESS"
echo "IOTA_NAMES_OBJECT_ID=$IOTA_NAMES_OBJECT_ID"
echo "IOTA_NAMES_PAYMENTS_PACKAGE_ADDRESS=$IOTA_NAMES_PAYMENTS_PACKAGE_ADDRESS"
echo "IOTA_NAMES_REGISTRY_ID=$IOTA_NAMES_REGISTRY_ID"
echo "IOTA_NAMES_REVERSE_REGISTRY_ID=$IOTA_NAMES_REVERSE_REGISTRY_ID"

cargo run --features=iota-names name register test.iota
