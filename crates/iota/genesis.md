# Genesis Ceremony

This document lays out the step-by-step process for orchestrating a Iota Genesis Ceremony.

## Prerequisites

Each validator participating in the ceremony will need the following:

- BLS12381 Public key
- Ed25519 Public key(s)
- Iota network address
- p2p_address network address
- Narwhal_primary_address network address
- Narwhal_worker_address network address

Note:

- Network addresses should be Multiaddrs in the form of `/dns/{dns name}/tcp/{port}/http` (for example `/dns/localhost/tcp/8080/http`).
- A BLS12381KeyPair can be created using `iota keytool generate bls12381`
- An Ed25519KeyPair can be created using `iota keytool generate ed25519`

## Ceremony

1. Creation of a shared workspace

To start, you'll need to create a shared workspace where all validators will be able to share their
information. For these instructions, we'll assume that such a shared workspace is created and managed
using a git repository hosted on git hosting provider.

The MC (Master of Ceremony) will create a new git repository and initialize the directory:

```
$ git init genesis && cd genesis
$ iota genesis-ceremony init
$ git add .
$ git commit -m "init genesis"
$ git push
```

2. Contribute Validator information

Once the shared workspace has been initialized, each validator can contribute their information:

```
$ git clone <url to genesis repo> && cd genesis
$ iota genesis-ceremony add-validator \
    --name <human-readable validator name> \
    --validator-key-file <BLS12381KeyPair VALIDATOR_KEY_FILE> \
    --worker-key-file <Ed25519KeyPair WORKER_KEY_FILE> \
    --account-key-file <Ed25519KeyPair ACCOUNT_KEY_FILE> \
    --network-key-file <Ed25519KeyPair NETWORK_KEY_FILE> \
    --network-address <multiaddr TCP> \
    --p2p-address <multiaddr UDP> \
    --narwhal-primary-address <multiaddr UDP> \
    --narwhal-worker-address <multiaddr UDP> \
    --description <validator description> \
    --image-url <validator image url> \
    --project-url <validator project url>

$ git add .
$ git commit -m "add validator <name>'s information"
$ git push # either to the shared workspace or another branch followed by a PR
```

3. Add token allocation for the faucet

Add allocation for any faucet that might have been launched.

```
$ iota genesis-ceremony add-token-allocation \
    --recipient-address <IotaAddress> \
    --amount-nanos <# of iota coins>
$ git add .
$ git commit -m "add faucet token allocation"
$ git push
```

4. Build Genesis

Once all validators and gas objects have been added, the MC can build the genesis object:

```
$ iota genesis-ceremony build-unsigned-checkpoint
$ git add .
$ git commit -m "build genesis"
$ git push
```

4. Verify and Sign Genesis

Once genesis is built each validator will need to verify and sign genesis:

```
$ iota genesis-ceremony verify-and-sign \
    --key-file <path to key file>
$ git add .
$ git commit -m "sign genesis"
$ git push
```

5. Finalize Genesis

Once all validators have successfully verified and signed genesis, the MC can finalize the ceremony
and then the genesis state can be distributed:

```
$ iota genesis-ceremony finalize
```
