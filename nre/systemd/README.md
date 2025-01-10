# Run a IOTA Node using Systemd

Tested using:

- Ubuntu 20.04 (linux/amd64) on bare metal
- Ubuntu 22.04 (linux/amd64) on bare metal

## Prerequisites and Setup

1. Add a `iota` user and the `/opt/iota` directories

```shell
sudo useradd iota
sudo mkdir -p /opt/iota/bin
sudo mkdir -p /opt/iota/config
sudo mkdir -p /opt/iota/db
sudo mkdir -p /opt/iota/key-pairs
sudo chown -R iota:iota /opt/iota
```

2. Install the IOTA Node (iota-node) binary, two options:

- Pre-built binary from [Github](https://github.com/iotaledger/iota/releases)

- Build from source (select the branch you want to build from depending on the network you want to connect to):

```shell
git clone https://github.com/iotaledger/iota.git && cd iota
git checkout [devnet|testnet|mainnet]
cargo build --release --bin iota-node
mv ./target/release/iota-node /opt/iota/bin/iota-node
```

3. Copy your key-pairs into `/opt/iota/key-pairs/`

Make sure when you copy them they retain `iota` user permissions. To be safe you can re-run: `sudo chown -R iota:iota /opt/iota`

4. Update the node configuration file and place it in the `/opt/iota/config/` directory.

Add the paths to your private keys to validator.yaml. If you chose to put them in `/opt/iota/key-pairs`, you can use the following example:

```
authority-key-pair: 
  path: /opt/iota/key-pairs/authority.key
protocol-key-pair: 
  path: /opt/iota/key-pairs/protocol.key
network-key-pair: 
  path: /opt/iota/key-pairs/network.key
```

5. Place `genesis.blob` (see [Genesis](https://docs.iota.org/operator/genesis)) in `/opt/iota/config/`

6. Copy the `iota-node` systemd service unit file

File: [iota-node.service](./iota-node.service)

Copy the file to `/etc/systemd/system/iota-node.service`.

7. Reload systemd with this new service unit file, run:

```shell
sudo systemctl daemon-reload
```

8. Enable the new service with systemd

```shell
sudo systemctl enable iota-node.service
```

## Connectivity

You may need to explicitly open the ports outlined in [IOTA for Node Operators](../validator-tasks#connectivity) for the required IOTA Node connectivity.

## Start the node

Start the Validator:

```shell
sudo systemctl start iota-node
```

Check that the node is up and running:

```shell
sudo systemctl status iota-node
```

Follow the logs with:

```shell
journalctl -u iota-node -f
```

## Updates

When an update is required to the IOTA Node software the following procedure can be used. It is highly **unlikely** that you will want to restart with a clean database.

- assumes iota-node lives in `/opt/iota/bin/`
- assumes systemd service is named `iota-node`
- **DO NOT** delete the IOTA databases

1. Stop iota-node systemd service

```
sudo systemctl stop iota-node
```

2. Fetch the new `iota-node` binary from [Github](https://github.com/iotaledger/iota/releases)

3. Update and move the new binary:

```
chmod +x iota-node
sudo mv iota-node /opt/iota/bin/
```

4. start iota-node systemd service

```
sudo systemctl start iota-node
```
