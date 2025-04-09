# IOTA Apps Backend

A lightweight backend for the web apps of IOTA.

# Set Up

**Requirements**: 20.0.0 or later.

Dependencies are managed using [`pnpm`](https://pnpm.io/). You can start by installing dependencies in the root of the iota repository:

```
$ pnpm install
```

> All `pnpm` commands below are intended to be run in the root of the iota repo.

## Build in watch mode (dev)

To build the backend and watch for changes run:

```
pnpm apps-backend dev
```

## Environment Variables

You can config default network and RPC endpoints by copying [sdk/.env.defaults]([sdk/.env.defaults) and rename it to `sdk/.env`.

For example, to change the default network from `localnet` to `testnet`, you can change `DEFAULT_NETWORK = 'localnet'` to `DEFAULT_NETWORK = 'testnet'`.

## Building for production

To build the apps-backend for production, run the following command:

```
pnpm apps-backend build
```

All build artifacts will go to [dist/](./dist/).

## Testing

```
pnpm apps-backend test:e2e
```
