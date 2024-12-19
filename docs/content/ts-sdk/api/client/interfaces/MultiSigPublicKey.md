# Interface: MultiSigPublicKey

The struct that contains the public key used for authenticating a MultiSig.

## Properties

### pk\_map

> **pk\_map**: [[`PublicKey`](../type-aliases/PublicKey.md), `number`][]

A list of public key and its corresponding weight.

---

### threshold

> **threshold**: `number`

If the total weight of the public keys corresponding to verified signatures is larger than
threshold, the MultiSig is verified.
