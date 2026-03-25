---
'@iota/iota-sdk': patch
---

Add `FormatBalanceOptions` interface to `formatBalance` function to support custom BigNumber formatting options. This enables disabling group separators for analytics and data processing use cases where plain numeric strings are required.
