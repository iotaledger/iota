import { IOTA_DECIMALS } from '@iota/iota-sdk/utils';
import { parseAmount } from './parseAmount';

export function buildDepositL2Parameters(receiverAddress: string, baseTokensToWithdraw: string) {
    const convertedBaseToken = Number(parseAmount(baseTokensToWithdraw, IOTA_DECIMALS));
    const parameters = [
        receiverAddress,
        {
            coins: [
                {
                    coinType:
                        '0x0000000000000000000000000000000000000000000000000000000000000002::iota::IOTA',
                    amount: convertedBaseToken,
                },
                // Put additional native tokens here with the proper coin type and value
            ],
            objects: [
                // Place any objects in here you want to withdraw
            ],
        },
    ];

    return parameters;
}
