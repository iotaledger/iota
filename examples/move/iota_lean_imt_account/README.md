# iota-lean-imt-account

An Abstract IOTA Account backed by a [Lean Incremental Merkle Tree](https://github.com/privacy-scaling-explorations/zk-kit.circom/issues/17) (LeanIMT). A set of IOTA addresses are hashed with [Poseidon](https://docs.rs/fastcrypto-zkp/latest/fastcrypto_zkp/) and inserted into the tree. Any address in the tree can authenticate as the account by submitting a [Groth16](https://docs.iota.org/developer/cryptography/on-chain/groth16) zero-knowledge proof of membership.

This enables shared accounts controlled by a large group of addresses without requiring individual on-chain transactions for each member, useful for airdrops, DAOs, or any scenario where many addresses need to act through a single account.

Two authentication modes are supported:

- **Secret mode** -- the caller proves membership without revealing their public key.
- **Public key mode** -- the caller's public key is disclosed on-chain and the leaf is derived from it.

> [!WARNING]\
> This is a PoC, as a properly secure design would involve at least some salt mechanism for the hash and additional proving mechanism; in here, if a public key being part of the IMT is disclosed, then it would be trivial to obtain an unwanted access to the account.

Check https://github.com/miker83z/iota-lean-imt-account for the instructions and code on how to generate a new tree and proofs.
