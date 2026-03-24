import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';
import { verifyPersonalMessageSignature } from '@iota/iota-sdk/verify';

const message = new TextEncoder().encode('hello world');
const zkSignature = ''; // In a real app, obtain this from a zkLogin signing flow

// The client can be used to fetch the zklogin address for verification
const _client = new IotaGraphQLClient({ url: 'https://graphql.testnet.iota.cafe' });
const publicKey = await verifyPersonalMessageSignature(message, zkSignature);
