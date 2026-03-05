import { KioskClient } from '@iota/kiosk';

declare const kioskClient: KioskClient;

const id = '0xKioskId';

const res = await kioskClient.getKiosk({
    id,
    options: {
        withKioskFields: true,
        withListingPrices: true,
    },
});
console.log(res);
