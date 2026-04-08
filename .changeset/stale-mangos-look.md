---
'@iota/dapp-kit': minor
---

Add cross-tab wallet state synchronization via a new `syncTabs` prop on `WalletProvider`.

When enabled, all open tabs automatically react to wallet connection changes made in any other tab - including connecting, switching accounts, and disconnecting - without requiring a page reload.

The feature is opt-in (disabled by default) to avoid breaking existing apps:

```tsx
<WalletProvider syncTabs={true}>
  {children}
</WalletProvider>
```