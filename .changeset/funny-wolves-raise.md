---
'@iota/dapp-kit': minor
---

Add a new `chain` prop for the `WalletProvider` so that you can globaly specify which chain do you want to sign with when using the `useSignAndExecuteTransaction` hook so that the wallet can properly dry run the transaction in the UI.
