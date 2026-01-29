// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function buildDepositL2Parameters(
    receiverAddress: string,
    amount: number | bigint,
    coinType: string,
) {
    const coins = [
        {
            coinType,
            amount,
        },
    ];

    const parameters = [
        receiverAddress,
        {
            coins,
            objects: [
                // Place any objects in here you want to withdraw
            ],
        },
    ];

    return parameters;
}
