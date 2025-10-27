---
description: How to setup an access node.
image: /img/logo/WASP_logo_dark.png
tags:
  - how-to
  - isc
---

# Running an ISC Access Node

As Wasp is dependent on a L1 Node, you must run the wasp node alongside your _IOTA node_. You can use the simple docker-compose setup to do so.

## Recommended Hardware Requirements

We recommend that you run the docker image on a server with:

- **CPU**: 8 core.
- **RAM**: 16 GB.
- **Disk space**: ~ 250 GB SSD, depending on your pruning configuration.

## Set Up

Clone and follow the instructions on the [wasp-docker-setup repo](https://github.com/iotaledger/wasp-docker-setup).

:::note
This is aimed at production-ready deployment. If you're looking to spawn a local node for testing/development, please see the [local-setup](https://github.com/iotaledger/wasp/tree/develop/tools/local-setup)
:::
