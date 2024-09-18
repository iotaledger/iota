// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getDefaultNetwork, Network } from '@iota/iota-sdk/client';

const walletDapps = [
    {
        name: 'IotaFrens',
        description: 'Enter the world of IotaFrens.',
        link: 'https://iotafrens.com/',
        icon: 'https://iotafrens.com/icons/favicon.ico',
        tags: ['Social'],
    },
    {
        name: 'Wormhole Connect',
        description:
            'Bridge tokens from any Wormhole supported chain into Iota and get dropped off with extra Iota to pay gas fees. Developers can also embed the Connect bridge directly into their own websites and Dapps.',
        link: 'https://www.portalbridge.com/iota',
        icon: 'https://www.portalbridge.com/favicon.ico',
        tags: ['DeFi'],
    },
    {
        name: 'Bullshark Quests',
        description: 'Earn rewards through engaging with apps on Iota!',
        link: 'https://quests.iota.org/',
        icon: 'https://user-images.githubusercontent.com/122397493/251579441-3c84de97-fc6e-46d2-b561-cd7bbef7dac7.png',
        tags: ['Social'],
    },
    {
        name: 'Aftermath Finance',
        description:
            'The all-in-one DEX on Iota, featuring a fully on-chain perpetuals exchange, smart-order routing, liquid staking, and a novel spot AMM.',
        link: 'https://aftermath.finance',
        icon: 'https://github-production-user-asset-6210df.s3.amazonaws.com/122397493/289710788-57f6935c-f930-4668-aa01-86a1579e406a.png',
        tags: ['DeFi', 'DEX', 'Infra'],
    },
    {
        name: 'NAVI',
        description: 'Native one stop liquidity protocol',
        link: 'https://app.naviprotocol.io',
        icon: 'https://www.naviprotocol.io/favicon.svg',
        tags: ['DeFi'],
    },
    {
        name: 'Scallop',
        description: 'Next generation Money Market',
        link: 'https://app.scallop.io/',
        icon: 'https://app.scallop.io/images/logo-192.png',
        tags: ['DeFi'],
    },
    {
        name: 'Typus',
        description: 'Real yield infrastructure',
        link: 'https://typus.finance/',
        icon: 'https://typus.finance/favicon.png',
        tags: ['DeFi'],
    },
    {
        name: 'Turbos',
        description: 'Non-custodial and user-centric DEX on Iota.',
        link: 'https://app.turbos.finance',
        icon: 'https://i.ibb.co/RTKw0PZ/Turbos-400x400.png',
        tags: ['DeFi'],
    },
    {
        name: 'Cetus Protocol',
        description: 'The pioneer concentrated liquidity DEX on Iota.',
        link: 'https://app.cetus.zone/',
        icon: 'https://i.ibb.co/TWdmdQz/cetus-icon.png',
        tags: ['DeFi', 'DEX'],
    },
    {
        name: 'FlowX Finance',
        description: 'Ecosystem-focused native DEX & aggregator on the Iota Network.',
        link: 'https://flowx.finance/swap',
        icon: 'https://3458959336-files.gitbook.io/~/files/v0/b/gitbook-x-prod.appspot.com/o/spaces%2FpEOgAVtfjtpMXMM550M8%2Fuploads%2FiunglJ6f4i7xbVyrPZTD%2FFlowX%20Logo%20Black.png?alt=media&token=6dfa923c-41f6-4252-831f-ac6d1ddd1afe',
        tags: ['DeFi', 'DEX'],
    },
    {
        name: 'Bucket Protocol',
        description: 'The first native stablecoin on Iota Network.',
        link: 'https://app.bucketprotocol.io/',
        icon: 'https://d3h53g0wjfwuec.cloudfront.net/bucket_avatar.png',
        tags: ['DeFi'],
    },
    {
        name: 'Aries Markets',
        description: 'Aries Markets - All-in-one DeFi platform with an innovative money market.',
        link: 'https://iota.ariesmarkets.xyz/',
        icon: 'https://iota.ariesmarkets.xyz/favicon.png',
        tags: ['DeFi'],
    },
    {
        name: 'Clutchy',
        description: 'Gaming and NFT marketplace for creators and communities',
        link: 'https://clutchy.io/',
        icon: 'https://clutchy.io/favicon.ico',
        tags: ['Marketplace'],
    },
    {
        name: 'Keepsake',
        description: 'One of the first NFT Marketplaces on Iota Network.',
        link: 'https://keepsake.gg/',
        icon: 'https://keepsake.gg/assets/icon/Favicon.png',
        tags: ['Marketplace'],
    },
];

export const featuresMock = {
    'mainnet-selection': {
        defaultValue: true,
    },
    'recognized-packages': {
        defaultValue: [
            '0x2',
            '0x3',
            '0xa198f3be41cda8c07b3bf3fee02263526e535d682499806979a111e88a5a8d0f',
            '0x027792d9fed7f9844eb4839566001bb6f6cb4804f66aa2da6fe1ee242d896881',
            '0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf',
            '0xe32d3ebafa42e6011b87ef1087bbc6053b499bf6f095807b9013aff5a6ecd7bb',
            '0x909cba62ce96d54de25bec9502de5ca7b4f28901747bbf96b76c2e63ec5f1cba',
            '0xcf72ec52c0f8ddead746252481fb44ff6e8485a39b803825bde6b00d77cdb0bb',
            '0xb231fcda8bbddb31f2ef02e6161444aec64a514e2c89279584ac9806ce9cf037',
            '0xc060006111016b8a020ad5b33834984a437aaa7d3c74c18e09a95d48aceab08c',
            '0x1e8b532cca6569cab9f9b9ebc73f8c13885012ade714729aa3b450e0339ac766',
            '0xb848cce11ef3a8f62eccea6eb5b35a12c4c2b1ee1af7755d02d7bd6218e8226f',
            '0x027792d9fed7f9844eb4839566001bb6f6cb4804f66aa2da6fe1ee242d896881',
            '0xaf8cd5edc19c4512f4259f0bee101a40d41ebed738ade5874359610ef8eeced5',
            '0x6081300950a4f1e2081580e919c210436a1bed49080502834950d31ee55a2396',
            '0x66f87084e49c38f76502d17f87d17f943f183bb94117561eb573e075fdc5ff75',
            '0xdbe380b13a6d0f5cdedd58de8f04625263f113b3f9db32b3e1983f49e2841676',
            '0xb7844e289a8410e50fb3ca48d69eb9cf29e27d223ef90353fe1bd8e27ff8f3f8',
            '0x6864a6f921804860930db6ddbe2e16acdf8504495ea7481637a1c8b9a8fe54b',
            '0x1d58e26e85fbf9ee8596872686da75544342487f95b1773be3c9a49ab1061b19',
            '0xe4239cd951f6c53d9c41e25270d80d31f925ad1655e5ba5b543843d4a66975ee',
            '0x5d1f47ea69bb0de31c313d7acf89b890dbb8991ea8e03c6c355171f84bb1ba4a',
            '0x94e7a8e71830d2b34b3edaa195dc24c45d142584f06fa257b73af753d766e690',
            '0xce7ff77a83ea0cb6fd39bd8748e2ec89a3f41e8efdc3f4eb123e0ca37b184db2',
            '0x9a5502414b5d51d01c8b5641db7436d789fa15a245694b24aa37c25c2a6ce001',
            '0xf325ce1300e8dac124071d3152c5c5ee6174914f8bc2161e88329cf579246efc',
            '0x6dae8ca14311574fdfe555524ea48558e3d1360d1607d1c7f98af867e3b7976c',
            '0x65ed6d4e666fcbc1afcd9d4b1d6d4af7def3eeeeaa663f5bebae8101112290f6',
            '0x76cb819b01abed502bee8a702b4c2d547532c12f25001c9dea795a5e631c26f1',
            '0x549e8b69270defbfafd4f94e17ec44cdbdd99820b33bda2278dea3b9a32d3f55',
            '0xbde4ba4c2e274a60ce15c1cfff9e5c42e41654ac8b6d906a57efa4bd3c29f47d',
            '0xb6baa75577e4bbffba70207651824606e51d38ae23aa94fb9fb700e0ecf50064',
            '0xa99b8952d4f7d947ea77fe0ecdcc9e5fc0bcab2841d6e2a5aa00c3044e5544b5',
            '0xc44d97a4bc4e5a33ca847b72b123172c88a6328196b71414f32c3070233604b2',
            '0x960b531667636f39e85867775f52f6b1f220a058c4de786905bdf761e06a56bb',
            '0x7016aae72cfc67f2fadf55769c0a7dd54291a583b63051a5ed71081cce836ac6',
            '0x06864a6f921804860930db6ddbe2e16acdf8504495ea7481637a1c8b9a8fe54b',
            '0x30a644c3485ee9b604f52165668895092191fcaf5489a846afa7fc11cdb9b24a',
        ],
    },
    'team-address-overrides': {
        defaultValue: {
            addresses: [
                '0x2fd42dfdbd2eb7055a7bc7d4ce000ae53cc22f0c2f2006862bebc8df1f676027',
                '0xba097bfb05f1af3b1b022ad4fe597bcce53ff068a323b901dae0e96f3af68a7d',
                '0xe0b97bff42fcef320b5f148db69033b9f689555348b2e90f1da72b0644fa37d0',
            ],
        },
    },
    'token-name-overrides': {
        defaultValue: {
            '0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN':
                'wUSDC (Eth)',
            '0x909cba62ce96d54de25bec9502de5ca7b4f28901747bbf96b76c2e63ec5f1cba::coin::COIN':
                'wUSDC (Bnb)',
            '0xb231fcda8bbddb31f2ef02e6161444aec64a514e2c89279584ac9806ce9cf037::coin::COIN':
                'wUSDC (Sol)',
            '0xe32d3ebafa42e6011b87ef1087bbc6053b499bf6f095807b9013aff5a6ecd7bb::coin::COIN':
                'wUSDC (Arb)',
            '0xcf72ec52c0f8ddead746252481fb44ff6e8485a39b803825bde6b00d77cdb0bb::coin::COIN':
                'wUSDC (Polygon)',
            '0x94e7a8e71830d2b34b3edaa195dc24c45d142584f06fa257b73af753d766e690::celer_usdc_coin::CELER_USDC_COIN':
                'USDC (Celer)',
        },
    },
    'wallet-released-version': {
        defaultValue: '0.0.0',
    },
    'enoki-self-serve-subscriptions': {
        defaultValue: false,
        rules: [
            {
                force: true,
            },
        ],
    },
    'wallet-staking-enabled': {
        defaultValue: false,
        rules: [
            {
                condition: {
                    version: {
                        $gte: '23.1.25.225',
                    },
                },
                force: true,
            },
        ],
    },
    'wallet-sentry-tracing': {
        defaultValue: 0.0025,
    },
    'wallet-dapps': {
        defaultValue: [],
        rules: [
            {
                condition: {
                    platform: {
                        $in: ['ios', 'android'],
                    },
                    network: getDefaultNetwork(),
                },
                force: walletDapps,
            },
            {
                condition: {
                    network: getDefaultNetwork(),
                },
                force: walletDapps,
            },
            {
                condition: {
                    network: Network.Testnet,
                },
                force: walletDapps,
            },
        ],
    },
    'wallet-multi-accounts': {
        defaultValue: false,
        rules: [
            {
                condition: {
                    version: {
                        $exists: true,
                        $gt: '23.3.1.1',
                    },
                },
                force: true,
            },
        ],
    },
    'wallet-ledger-integration': {
        defaultValue: false,
        rules: [
            {
                force: true,
            },
        ],
    },
    'wallet-balance-refetch-interval': {
        defaultValue: 1000,
    },
    'wallet-activity-refetch-interval': {
        defaultValue: 1000,
    },
    'wallet-ledger-notification-enabled': {
        defaultValue: false,
        rules: [
            {
                force: true,
            },
        ],
    },
    'kiosk-marketplace-links': {
        defaultValue: [
            {
                href: 'https://docs.iota.io/build/iota-kiosk',
                text: 'Learn more about Kiosks',
            },
            {
                href: 'https://iota.hyperspace.xyz',
                text: 'Marketplace',
            },
        ],
    },
    'wallet-bullshark-interstitial': {
        defaultValue: true,
    },
    'kiosk-originbyte-packageid': {
        defaultValue: '0x2678c98fe23173eebea384509464eb81b1f3035a57419cb46d025000c337451a',
    },
    'wallet-apps-banner-config': {
        defaultValue: {
            enabled: true,
            bannerUrl: 'https://iotawallet.com',
            imageUrl: 'https://fe-assets.iota.org/wallet-next/iotawallet-mobile.svg',
        },
        rules: [
            {
                condition: {
                    platform: {
                        $in: ['ios', 'android'],
                    },
                },
                force: {
                    enabled: false,
                    bannerUrl: 'https://iota.io/basecamp',
                    imageUrl: 'https://fe-assets.iota.org/basecamp/wallet_basecamp_banner.png',
                },
            },
        ],
    },
    'wallet-interstitial-config': {
        defaultValue: {
            enabled: false,
            dismissKey: 'winter-quest',
            imageUrl: 'https://fe-assets.iota.org/quests/winter-apps-interstitial.svg',
            bannerUrl: 'https://www.blog.iota.org/winter-quest',
        },
    },
    'wallet-defi': {
        defaultValue: true,
        rules: [
            {
                force: true,
            },
        ],
    },
    'wallet-fee-address': {
        defaultValue: '0x55b0eb986766351d802ac3e1bbb8750a072b3fa40c782ebe3a0f48c9099f7fd3',
    },
    'deep-book-configs': {
        defaultValue: {
            pools: {
                IOTA_USDC: [
                    '0x4405b50d791fd3346754e8171aaab6bc2ed26c2c46efdd033c14b30ae507ac33',
                    '0x7f526b1263c4b91b43c9e646419b5696f424de28dda3c1e6658cc0a54558baa7',
                ],
                WETH_USDC: ['0xd9e45ab5440d61cc52e3b2bd915cdd643146f7593d587c715bc7bfa48311d826'],
                TBTC_USDC: ['0xf0f663cf87f1eb124da2fc9be813e0ce262146f3df60bc2052d738eb41a25899'],
                USDT_USDC: ['0x5deafda22b6b86127ea4299503362638bea0ca33bb212ea3a67b029356b8b955'],
            },
            coinsMap: {
                IOTA: '0x0000000000000000000000000000000000000000000000000000000000000002::iota::IOTA',
                USDC: '0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN',
                USDT: '0xc060006111016b8a020ad5b33834984a437aaa7d3c74c18e09a95d48aceab08c::coin::COIN',
                WETH: '0xaf8cd5edc19c4512f4259f0bee101a40d41ebed738ade5874359610ef8eeced5::coin::COIN',
                TBTC: '0xbc3a676894871284b3ccfb2eec66f428612000e2a6e6d23f592ce8833c27c973::coin::COIN',
            },
        },
    },
    'wallet-qr-scanner': {
        defaultValue: false,
    },
    'defi-max-slippage': {
        defaultValue: 1,
    },
    'lst-max-slippage': {
        defaultValue: 10,
    },
    domainregistration: {
        defaultValue: true,
        rules: [
            {
                condition: {
                    network: {
                        $ne: 'mainnet',
                    },
                },
                force: true,
            },
        ],
    },
    'enable-notifi': {
        defaultValue: false,
        rules: [
            {
                condition: {
                    network: {
                        $ne: 'mainnet',
                    },
                },
                force: true,
            },
        ],
    },
    expiration_period: {
        defaultValue: 30,
        rules: [
            {
                condition: {
                    network: {
                        $ne: 'mainnet',
                    },
                },
                force: 120,
            },
        ],
    },
    'validator-page-staking': {
        defaultValue: true,
    },
    'polling-txn-table': {
        defaultValue: true,
    },
    'network-outage-override': {
        defaultValue: false,
    },
    'module-source-verification': {
        defaultValue: true,
    },
    'explorer-redirect': {
        defaultValue: true,
    },
    'iotafrens-enable-nhn-accessories': {
        defaultValue: false,
        rules: [
            {
                force: true,
            },
        ],
    },
    'claim-free-iotafren-hat-promo': {
        defaultValue: true,
    },
    'iotafrens-enable-babyshark-banner': {
        defaultValue: false,
        rules: [
            {
                force: true,
            },
        ],
    },
    'iotafrens-enable-babyshark-banner-basecamp-accessories': {
        defaultValue: false,
        rules: [
            {
                force: true,
            },
        ],
    },
};
