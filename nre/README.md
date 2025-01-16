# Node and Network Reliability Engineering

---

This repo contains:

- [IOTA for Node Operators](./iota_for_node_operators.md) - This documentation aggregates all the information about deploying and operating the IOTA Node software for Node Operators.

- `ansible/` - An ansible playbook for standing up your node. Successful execution of the playbook will result in a systemd managed process running iota-node. You can use this or just consult the steps when provisioning your node.

- `config/` - IOTA Node configuration templates.

- `docker/` - A docker compose configuration for standing up your node. You can use this or just consult the steps when provisioning your node.

- `systemd/` - Steps to setup your node as a systemd service. You can use this or reference the steps when provisioning your node.
