export function buildDepositL2Parameters(
    receiverAddress: string,
    coinBalances: { coinType: string; amount: number | bigint }[],
) {
    const parameters = [
        receiverAddress,
        {
            coins: coinBalances,
            objects: [
                // Place any objects in here you want to withdraw
            ],
        },
    ];

    return parameters;
}
