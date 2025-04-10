# Wallet dashboard

# Set Up

**Requirements**: Node 20.0.0 or later.

Dependencies are managed using [`pnpm`](https://pnpm.io/). You can start by installing dependencies in the root of the repository:

```
$ pnpm install
```

> All `pnpm` commands below are intended to be run in the root of the repo.

## Developing the Wallet Dashboard

To start the wallet dashboard dev server, you can run the following command:

```
pnpm wallet-dashboard dev
```

This will start the dev server on port 3000, which should be accessible on http://localhost:3000/

## To run end-to-end localnet test

Start validators locally:

```bash
cargo run --bin iota start --force-regenesis --with-faucet
```

In a separate terminal, you can now run the end-to-end tests:

```bash
pnpm --filter wallet-dashboard playwright test
```
