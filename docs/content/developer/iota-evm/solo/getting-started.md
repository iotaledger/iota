---
description: Solo is a testing framework that allows developers to validate real smart contracts and entire inter-chain protocols.
image: /img/logo/WASP_logo_dark.png
tags:
  - how-to
  - evm
  - solo
  - testing
teams:
  - iotaledger/l2-smart-contract
---

# Testing Smart Contracts with Solo

## What is Solo?

[_Solo_](https://github.com/iotaledger/wasp/tree/develop/packages/solo) is a testing framework that allows developers to
validate real smart contracts and entire inter-chain protocols before deploying them on the distributed network.

## Installation

### Prerequisites

- [Go (version 1.20)](https://tip.golang.org/doc/go1.20). As _Solo_ tests are written in Go, you must
  [install Go](https://go.dev/doc/install).

- [Docker](https://www.docker.com/). Solo will run a local IOTA network in a Docker container

### Access the Solo Framework

You can access the Solo package by cloning the [Wasp repository](#clone-the-wasp-repository)
or [installing the Solo package](#install-the-solo-package).

#### Clone the Wasp Repository

_Solo_ is part of the [_Wasp_ codebase repository](https://github.com/iotaledger/wasp.git). You can access the Solo
framework by cloning the repository with the following command:

```shell
git clone https://github.com/iotaledger/wasp.git
```

After you have cloned the repository, you can access the Solo package in the `/packages/solo` folder.

#### Install the Solo Package

You can install the Solo package separately using the following command:

```shell
go get github.com/iotaledger/wasp/packages/solo
```

:::tip Go Docs

You can browse the Solo Go API reference (updated to the `master` branch) in
[go-docs](https://pkg.go.dev/github.com/iotaledger/wasp/packages/solo).

:::
