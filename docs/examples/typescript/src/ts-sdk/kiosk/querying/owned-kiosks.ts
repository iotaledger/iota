import { KioskClient } from '@iota/kiosk';

declare const kioskClient: KioskClient;

const address = '0xAddress';
const { kioskOwnerCaps, kioskIds } = await kioskClient.getOwnedKiosks({ address });
console.log(kioskOwnerCaps);
