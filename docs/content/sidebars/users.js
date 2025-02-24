const users = [
    {
        type: 'category',
        label: 'IOTA Wallet',
        items: [
            'about-iota/iota-wallet/getting-started',
            {
                type: 'category',
                label: 'How To',
                items: [
                    'about-iota/iota-wallet/how-to/basics',
                    'about-iota/iota-wallet/how-to/stake',
                    'about-iota/iota-wallet/how-to/multi-account',
                    'about-iota/iota-wallet/how-to/get-test-tokens',
                    'about-iota/iota-wallet/how-to/integrate-ledger',
                ],
            },
            'about-iota/iota-wallet/FAQ',
        ],
    },
];
module.exports = users;
