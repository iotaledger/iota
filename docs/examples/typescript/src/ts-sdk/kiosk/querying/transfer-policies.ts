import { KioskClient } from '@iota/kiosk';

declare const kioskClient: KioskClient;

const itemType = '0xAddress::hero::Hero';
const policies = await kioskClient.getTransferPolicies({ type: itemType });
console.log(policies);
