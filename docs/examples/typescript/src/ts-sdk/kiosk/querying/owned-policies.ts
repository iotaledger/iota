import { KioskClient } from '@iota/kiosk';

declare const kioskClient: KioskClient;

const address = '0xAddress';
const policies = await kioskClient.getOwnedTransferPolicies({ address });
console.log(policies);
