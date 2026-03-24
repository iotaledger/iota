import { KioskClient } from '@iota/kiosk';
import { IotaClient, Network } from '@iota/iota-sdk/client';

const client = new IotaClient({ url: 'https://example.com' });

const kioskClient = new KioskClient({
    client,
    network: Network.Custom,
    packageIds: {
        kioskLockRulePackageId: '0x...',
        royaltyRulePackageId: '0x...',
        personalKioskRulePackageId: '0x...',
        floorPriceRulePackageId: '0x...',
    },
});
