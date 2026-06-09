const users = [
  {
    type: 'category',
    label: 'Wallets',
    link: {
      type: 'generated-index',
      title: 'IOTA Wallets',
      description: 'Learn about the different wallets available for IOTA.',
      slug: '/users',
    },
    items: [
      {
        type: 'category',
        label: 'IOTA Wallet',
        description: 'The official IOTA Wallet.',
        items: [
          'users/iota-wallet/getting-started',
          {
            type: 'category',
            label: 'How To',
            items: [
              'users/iota-wallet/how-to/basics',
              'users/iota-wallet/how-to/stake',
              'users/iota-wallet/how-to/import',
              {
                type: 'category',
                label: 'Import Method',
                items: [
                  'users/iota-wallet/how-to/import/ledger',
                  'users/iota-wallet/how-to/import/keystone',
                  'users/iota-wallet/how-to/import/mnemonic',
                  'users/iota-wallet/how-to/import/seed',
                  'users/iota-wallet/how-to/import/passkey',
                  'users/iota-wallet/how-to/import/balance-finder',
                  'users/iota-wallet/how-to/import/migration',
                ],
              },
              'users/iota-wallet/how-to/multi-account',
              'users/iota-wallet/how-to/get-test-tokens',
            ],
          },
          'users/iota-wallet/FAQ',
        ],
      },
      {
        type: 'link',
        label: 'Nightly Wallet',
        href: 'https://nightly.app/download',
        description: 'Nightly provides a browser extension and mobile app for IOTA.',
      },
      {
        type: 'link',
        label: 'Cosmostation Wallet',
        href: 'https://www.cosmostation.io/products/application',
        description: 'Cosmostation provides a browser extension and mobile app for IOTA.',
      }
    ],
  },
  {
    type: 'category',
    label: 'IOTA Wallet Dashboard',
    items: [
      'users/iota-wallet-dashboard/getting-started',
      {
        type: 'category',
        label: 'How To',
        items: [
          'users/iota-wallet-dashboard/how-to/basics',
          'users/iota-wallet-dashboard/how-to/assets',
          'users/iota-wallet-dashboard/how-to/stake',
          'users/iota-wallet-dashboard/how-to/vesting',
          'users/iota-wallet-dashboard/how-to/migration',
        ],
      },
    ],
  },
  'users/legacy-migration-tool',
];

module.exports = users;
