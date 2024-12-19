# Interface: MultiSig

The struct that contains signatures and public keys necessary for authenticating a MultiSig.

## Properties

### bitmap

> **bitmap**: `number`

A bitmap that indicates the position of which public key the signature should be authenticated with.

---

### multisig\_pk

> **multisig\_pk**: [`MultiSigPublicKey`](MultiSigPublicKey.md)

The public key encoded with each public key with its signature scheme used along with the
corresponding weight.

---

### sigs

> **sigs**: [`CompressedSignature`](../type-aliases/CompressedSignature.md)[]

The plain signature encoded with signature scheme.
