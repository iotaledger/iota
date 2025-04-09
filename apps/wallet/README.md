# IOTA Wallet

A Chrome extension wallet for [IOTA](https://iota.org).

# Set Up

**Requirements**: 20.0.0 or later.

Dependencies are managed using [`pnpm`](https://pnpm.io/). You can start by installing dependencies in the root of the iota repository:

```
$ pnpm install
```

> All `pnpm` commands below are intended to be run in the root of the iota repo.

## Build in watch mode (dev)

To build the extension and watch for changes run:

```
pnpm wallet dev
```

This will build the app in the [dist/](./dist/) directory, watch for changes and rebuild it. (Also runs prettier to format the files that changed.)

## Environment Variables

You can config default network and RPC endpoints by copying [sdk/.env.defaults]([sdk/.env.defaults) and rename it to `sdk/.env`.

For example, to change the default network from `localnet` to `testnet`, you can change `DEFAULT_NETWORK = 'localnet'` to `DEFAULT_NETWORK = 'testnet'`.

## Building the wallet

To build the app, run the following command:

```
pnpm wallet build
```

The output directory is the same [dist/](./dist/), all build artifacts will go there

## Install the extension to Chrome

After building the app, the extension needs to be installed to Chrome. Follow the steps to [load an unpacked extension](https://developer.chrome.com/docs/extensions/get-started/tutorial/hello-world#load-unpacked) and install the app from the [dist/](./dist/) directory.

## Testing

```
pnpm wallet test
```
