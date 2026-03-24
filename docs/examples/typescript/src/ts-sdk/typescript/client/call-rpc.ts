import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';

const client = new IotaClient({ url: getFullnodeUrl('devnet') });

const committeeInfo = await client.call('iotax_getCommitteeInfo', []);
