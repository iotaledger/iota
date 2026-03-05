import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';
import { verifyPersonalMessageSignature } from '@iota/iota-sdk/verify';

declare const message: Uint8Array;
declare const zkSignature: string;

// The client can be used to fetch the zklogin address for verification
const _client = new IotaGraphQLClient({ url: 'https://graphql.testnet.iota.cafe' });
const publicKey = await verifyPersonalMessageSignature(message, zkSignature);
