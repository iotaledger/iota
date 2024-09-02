# IotaNS TypeScript SDK

This is a lightweight SDK (1kB minified bundle size), providing utility classes and functions for
applications to interact with on-chain `.iota` names registered from
[Iota Name Service (iotans.io)](https://iotans.io).

## Getting started

The SDK is published to [npm registry](https://www.npmjs.com/package/@iota/iotans-toolkit). To use
it in your project:

```bash
$ npm install @iota/iotans-toolkit
```

You can also use yarn or pnpm.

## Examples

Create an instance of IotansClient:

```typescript
import { IotaClient } from '@iota/iota-sdk/client';
import { IotansClient } from '@iota/iotans-toolkit';

const client = new IotaClient();
export const iotansClient = new IotansClient(client);
```

Choose network type:

```typescript
export const iotansClient = new IotansClient(client, {
    networkType: 'testnet',
});
```

> **Note:** To ensure best performance, please make sure to create only one instance of the
> IotansClient class in your application. Then, import the created `iotansClient` instance to use
> its functions.

Fetch an address linked to a name:

```typescript
const address = await iotansClient.getAddress('iotans.iota');
```

Fetch the default name of an address:

```typescript
const defaultName = await iotansClient.getName(
    '0xc2f08b6490b87610629673e76bab7e821fe8589c7ea6e752ea5dac2a4d371b41',
);
```

Fetch a name object:

```typescript
const nameObject = await iotansClient.getNameObject('iotans.iota');
```

Fetch a name object including the owner:

```typescript
const nameObject = await iotansClient.getNameObject('iotans.iota', {
    showOwner: true,
});
```

Fetch a name object including the Avatar the owner has set (it automatically includes owner too):

```typescript
const nameObject = await iotansClient.getNameObject('iotans.iota', {
    showOwner: true, // this can be skipped as showAvatar includes it by default
    showAvatar: true,
});
```

## License

[Apache-2.0](https://github.com/IotaNSdapp/toolkit/blob/main/LICENSE)
