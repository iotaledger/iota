---
'@iota/graphql-transport': minor
'@iota/iota-sdk': minor
'@iota/kiosk': minor
---

Rename `getLatestIotaSystemState` to `getLatestIotaSystemStateV1` and add a new backwards-compatible and future-proof `getLatestIotaSystemState` method that dinamically calls ``getLatestIotaSystemStateV1`or`getLatestIotaSystemStateV2` based on the protocol version of the node.
