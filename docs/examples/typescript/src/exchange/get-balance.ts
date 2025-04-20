import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';

async function run() {
    const iotaClient = new IotaClient({ url: getFullnodeUrl('devnet') });

    const MY_ADDRESS = '0x849d63687330447431a2e76fecca4f3c10f6884ebaa9909674123c6c662612a3';

    const balance = await iotaClient.getBalance({
        owner: MY_ADDRESS,
    });

    console.log('Balance in Nano (1_000_000_000 Nano = 1 IOTA): ', balance.totalBalance);
}

run().then(() => process.exit());
