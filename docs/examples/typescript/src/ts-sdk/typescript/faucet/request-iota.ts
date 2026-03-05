import { getFaucetHost, requestIotaFromFaucetV1 } from '@iota/iota-sdk/faucet';

await requestIotaFromFaucetV1({
    host: getFaucetHost('testnet'),
    recipient: '<RECIPIENT_ADDRESS>',
});
