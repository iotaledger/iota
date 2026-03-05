import { KioskClient } from '@iota/kiosk';

declare const kioskClient: KioskClient;

const type = '0xAddress::custom_extension::ACustomExtensionType';

const extension = await kioskClient.getKioskExtension({
    kioskId: '0xAKioskId',
    type,
});

console.log(extension);
