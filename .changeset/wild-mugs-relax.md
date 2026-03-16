---
'@iota/iota-sdk': patch
---

Fix system objects to avoid unnecessary network calls

- Clock (0x6) and Random (0x8) now return fully resolved SharedObject references with mutable: false
- System (0x5) and DenyList (0x403) now accept optional `mutable` parameter:
  - When undefined, returns UnresolvedObject with initialSharedVersion for backward compatibility
  - When explicitly set, returns SharedObjectRef with the specified mutability
- Improves transaction building performance by avoiding unnecessary network lookups
