# @iota/examples

Runnable examples demonstrating common operations with the IOTA TypeScript SDK.

## Examples

| Example              | Description                                                         |
| -------------------- | ------------------------------------------------------------------- |
| `custom-signer.ts`   | Extend the abstract `Signer` class and use it to sign a transaction |
| `get-balance.ts`     | Fetch an account's IOTA balance and coin objects from devnet        |
| `transfer-iota.ts`   | Build and execute a transaction to transfer IOTA tokens on devnet   |
| `tx-with-graphql.ts` | Execute a transaction and query details via GraphQL transport       |

## Running

From the package directory, run any example with:

```bash
pnpm example ./src/<file>.ts
```

For instance:

```bash
pnpm example ./src/get-balance.ts
```
