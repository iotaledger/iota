# Configure a Linux system as an IOTA Node using Ansible

This is a self-contained Ansible role for configuring a Linux system as an IOTA Node.

Tested with `ansible [core 2.13.4]` and:

- ubuntu 20.04 (linux/amd64) on bare metal
- ubuntu 22.04 (linux/amd64) on bare metal

## Prerequisites and Setup

1. Install [Ansible](https://docs.ansible.com/ansible/latest/installation_guide/intro_installation.html)

2. Add the target host to the [Ansible Inventory](./inventory.yaml)

3. Update the `iota_release` var in the [Ansible Inventory](./inventory.yaml)

4. Update [validator.yaml](../config/validator.yaml) and copy it to this directory.

5. Copy the `genesis.blob` to this directory (see [Genesis](https://docs.iota.org/operator/genesis)).

6. Update the `keypair_path` var in the [Ansible Inventory](./inventory.yaml)

## Example use:

- Configure everything:

`ansible-playbook -i inventory.yaml iota-node.yaml -e host=$inventory_name`

- Software update:

`TODO`
