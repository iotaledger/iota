import { getFaucetHost, requestIotaFromFaucetV1 } from '@iota/iota-sdk/faucet';

await requestIotaFromFaucetV1({
    host: getFaucetHost('devnet'),
    recipient: '<YOUR IOTA ADDRESS>',
});
