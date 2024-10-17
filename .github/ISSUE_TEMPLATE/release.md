---
name: Release
about: Request a new release
title: '[Task(*)]: release version x.y.z'
---

```[tasklist]
### Tasks
- [ ] Edit `Cargo.toml`
- [ ] Edit `crates/iota-open-rpc/spec/openrpc.json`
- [ ] Update `sdk/typescript/src/version.ts` (`pnpm sdk build`)
- [ ] Update `Cargo.lock` (`cargo check`)
- [ ] Update `crates/iota-framework/packages/deepbook/Move.lock`
- [ ] Update `crates/iota-framework/packages/iota-framework/Move.lock`
- [ ] Update `crates/iota-framework/packages/iota-system/Move.lock`
- [ ] Update `crates/iota-framework/packages/move-stdlib/Move.lock`
- [ ] Update `crates/iota-framework/packages/stardust/Move.lock`
```
