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
