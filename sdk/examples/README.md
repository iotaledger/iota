# @iota/examples

Runnable examples demonstrating common operations with the IOTA TypeScript SDK.

## Examples

| Example                          | Description                                                            |
| -------------------------------- | ---------------------------------------------------------------------- |
| `get-balance.ts`                 | Fetch an account's IOTA balance and coin objects from devnet           |
| `transfer-iota.ts`               | Build and execute a transaction to transfer IOTA tokens on devnet      |
| `tx-with-graphql.ts`             | Execute a transaction and query details via GraphQL transport          |
| `move-authenticator.ts`          | Publish, create and send with an on-chain account (Move Authenticator) |
| `move-authenticator-existing.ts` | Send with an on-chain account (Move Authenticator)                     |

## Running

From the package directory, run any example with:

```bash
pnpm example ./src/<file>.ts
```

For instance:

```bash
pnpm example ./src/get-balance.ts
```
