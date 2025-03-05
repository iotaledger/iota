---
'@iota/iota-sdk': minor
---

Updated `normalizeIotaAddress` function to: properly handle multiple '0x' prefixes at the beginning of addresses; trim whitespace from input addresses; throw error for non-hexadecimal characters;
