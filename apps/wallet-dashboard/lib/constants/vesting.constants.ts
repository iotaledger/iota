// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    DAYS_PER_WEEK,
    MILLISECONDS_PER_DAY,
    DAYS_PER_YEAR,
} from '@iota/core/constants/time.constants';
import { TimelockedObject } from '../interfaces';
import {
    getMockedSupplyIncreaseVestingTimelockedObjectsWithDynamicDate,
    getMockedVestingTimelockedStakedObjectsWithDynamicDate,
} from '../utils/vesting/buildMockedObjectsWithDynamicDateUtils';
import { DelegatedTimelockedStake } from '@iota/iota-sdk/client';

export const SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE = 2 * DAYS_PER_WEEK;
export const SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE_MILLISECONDS =
    SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE * MILLISECONDS_PER_DAY;
export const SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR = Math.round(
    DAYS_PER_YEAR / SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE,
);
export const SUPPLY_INCREASE_STARTING_VESTING_YEAR: number = 2023;
export const SUPPLY_INCREASE_STAKER_VESTING_DURATION = 2; // Years
export const SUPPLY_INCREASE_INVESTOR_VESTING_DURATION = 4; // Years

export const MIN_STAKING_THRESHOLD = 1_000_000_000;

// https://github.com/iotaledger/iota/blob/b0db487868fd5d61241a43eb8bc9886d7c1be1c9/crates/iota-types/src/timelock/stardust_upgrade_label.rs#L12
export const SUPPLY_INCREASE_VESTING_LABEL =
    '000000000000000000000000000000000000000000000000000000000000107a::stardust_upgrade_label::STARDUST_UPGRADE_LABEL';

export const MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS: TimelockedObject[] = [
    {
        id: {
            id: '0xfe755ca67e3a0714f97ec3c49cfc6f3ecdab2673d96b5840294d3a5db376c99',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1697320800000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x682d14613231dd1dde39397977cdfafb6b6263b5683b6782348c597c104b834',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1698530400000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x93f2bf2d044e45e1a85c010c22357892d1625436b8c95b26dcdb6f309319064',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1699740000000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x44fa510ba216cd555ecd6b99d1ebd612f82e2bf421091c973bca49b064dc72b',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1700949600000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0xacd861b6dc5d108af03655a2175545ac6d432c526bcbe294b90e722fa36b459',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1702159200000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x8f9eeb5953c77d53dcff3057619af7a29be1d9ce67bf66c86ad5309379d17e5',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1703368800000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x70b1063c1104760afc06df5217bebdf02f937e1aff51211fc0472e677ba8c74',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1704578400000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0xb0aa6f655d08f630c15a2cfb4e3e13e307ce9d96c52c1e91c65a71a204819bd',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1705788000000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x65224b9a3b9eadc55be4cb6efa363f283b924607496d60c02deef2aa6bf9e22',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1706997600000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x68f9a2af0ebd0bcd9e3cc836ac7103670a9602e8dca8fd28e7b2b5a693898f2',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1708207200000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x637e6b758efdb8d49ee96397ca909d579bb77b79f8b64e7e7f1af13ad4f7ce4',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1709416800000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0xbd0f349c21b67faec992b6c9a1b9b6343b4ff1f2ad5f33b0b4cd0fc31be2b31',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1710626400000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0xfb8c3539b22e4086bd03417027e70515e6fb6d18f366876ad5ad0d8da3bde0f',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1711836000000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0xbfb7c1a941885cc55a191e579c7c6d5dc345d6b5b9cfa439f724a343d354032',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1713045600000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x8935a904f90e23f6f453cb0c85a03859e07f1c9e5a5d1644b2fbe7005d8e158',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1714255200000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x73be6f8df4b73b83f8ccf909d61aabb56c56c56aa597d2806eccf3ab4fac66b',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1715464800000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x20075cc2ebd5fa6e069829e58e55e6e010ad115e8cbc48d7a3d98d079ce649a',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1716674400000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0xce03433d496cb231ead90a661fe08b924eb9b0cfb43dd560ea02a8060f6afd0',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1717884000000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0xf111b8705ba276f8c6b76bdf72a4a46889cb8207cc5a80d3df0f40d9576116a',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1719093600000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0xbc27940fb9c6f96ae9e2c11ad151446e30de5281172e48aac7f600d1da92c10',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1720303200000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x016fae8797d3d12a26e215ec1815ee8adce70bb93149b4d55eb06a81c476ff9',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1721512800000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x06f1e354ff551d76da8dc890eab728a65319defb3608991b4c70a1a2b30e8f1',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1722722400000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0xc4cf3ea32480aab7d78784c6f00b9210ce0ffaabbcbb8cddd846073e7455386',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1723932000000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x6dc10a8008855549b8d92e7704c799253a953d9835af001970426414fdd3ba7',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1725141600000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0xa5f7a66c575db3f74c5fe7043c28f7231a2127aec4dc2de88f5b9d3cf020511',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1726351200000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0xde0a4c2e0f16541983302c596339815ffa4d4743509e8115bc06fcf7f71ea8f',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1727560800000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0xccc5d23ab69789b934b9bf7f5006e43eef45c2d7a251e3eec8b7dd24bc20a07',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1728770400000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x503dc8844b0cd6e74e735433751328e8283569e81b4602aaa6941ce3fe826bb',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1729980000000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x0fac98b5ac955644dffa0700933aababe438fae6fc58b8a4bd1f740c8aba941',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1731189600000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x756483e3c7dd3491ea405f682df6c5dc1e4a59d8b5c9725b0d194815a25ea95',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1732399200000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x72c4318876f51bed94c2228b395d18f5dce5f243039c7e3d8fad690dfe918fc',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1733608800000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x37f68fd72af05b4c923268b64a0baa7511f27bc4cbd90641e444e7116f02604',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1734818400000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
    {
        id: {
            id: '0x97bedf66e48392a0b9baf8a8280e72fcce9b32ff980832edfe1a90a14ce9047',
        },
        locked: {
            value: 2_000_000_000n,
        },
        expirationTimestampMs: 1736028000000,
        label: SUPPLY_INCREASE_VESTING_LABEL,
    },
];

export const MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS: DelegatedTimelockedStake[] = [
    {
        validatorAddress: '0x026d4a3c4bcdea163cef95220a0b8cd9a05995a0f9354730ea8d88e0b7635e66',
        stakingPool: '0xf59a3e927d5f8dd59b104327a59f48b73eda478aa60c983de1d0b6c7551734f4',
        stakes: [
            {
                timelockedStakedIotaId:
                    '0x2a1df8ec18ef82da39f8af22ae1b8656037706df377ad4af0fc2036f50373f1d',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1699740000000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x75e69abfc76ad38944e747f36ecf0dfd0933f80134187c7a67952f0011623b21',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1700949600000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x6c9abc5d279d79f1693f09fa220300ef8483bcbdcca410e3d533e4892d7a60f9',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1702159200000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x40d2fcada5c4b87854b458115d678d87317bf14b28abce0ae94be2063a5c9c0f',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1703368800000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0xfa414fb4078c7424f353a0206a0d18a07a21ff5ccc99e81fd15cf201fd0a65d4',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1704578400000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x67564a23c8a07f02755c8f23d3d97ed23de5f1af1b702e23e0fe6d5b68592334',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1705788000000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
        ],
    },
    {
        validatorAddress: '0xc556e3e84b39f730d6fd7ea152d2f947526b45c989a03633e5a79186fe52a3a0',
        stakingPool: '0x1d981f9fde96c2e509de1c925a95b78b2a3cb910d9b384ca4dbeb1bd14aa1cf2',
        stakes: [
            {
                timelockedStakedIotaId:
                    '0xd2f93f458c41ace2099f877f97233fc84f04eafbfb5a48b39ef15896bf34dcdb',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1706997600000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x38847bb6e80fc93d2a1924e65f37aa7f39c2b66c9cd0465cba4f8f7a2aa69cf4',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1708207200000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0xe9c230f1046e3460d38ff70c46aa7e4b812d797e81d91e6048966dd457516908',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1709416800000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x584049fcd0854e2d9ae7f5442ddcd7a6774941cea32f446baa42ed471c0c9b5e',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1710626400000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x0fce2e04c142b904eddd7b644161335387e0c76398add8d4b75af8b973eb06c1',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1711836000000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x8b3145f22980a2d506aa4d179657ca3acf2196509b4982d06d6c6a1cc033d47c',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1713045600000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
        ],
    },
    {
        validatorAddress: '0x69adfd0c384f62e1d56b4658521b84c3343418187fb3b53fd8836ec20c294477',
        stakingPool: '0xddd255ac76d01579d2d873cc0b0548ad58a11c18ac41c75c03aa0339890ef6ac',
        stakes: [
            {
                timelockedStakedIotaId:
                    '0x687a70de2a11071592da1e1c7e65530407577974a253fbb291ab694fa1862556',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1714255200000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0xcb5b7b159752b1854c368d2f178f92e579a27c64a882669ca8ffddb921d5934e',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1715464800000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x9d44592883927293bd177924cca351bc7c6c075a834d44711bb5f85cebd47cb9',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1716674400000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x72b8c7e5695e86b043b4f66202687fe7d1a11a0118bc795133f2af2f5229b4c7',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1717884000000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x7bbd702f91697c81e05a17c6c6cca7160032627e1ca3af2736fd826f42196ff7',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1719093600000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0xaf4544a2086985b0c29fee18df4f2fe616370824ef6d62c2965615c74f53fbee',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1720303200000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
        ],
    },
    {
        validatorAddress: '0x81c7661212e17f6acd3620a4f4191b350a350b6fdabdefdd7f0940b962f5e6e3',
        stakingPool: '0xea1a6f7ff4c03ce2a56687716d9c6e373286d2dca12cc0a4a86c2b943173553c',
        stakes: [
            {
                timelockedStakedIotaId:
                    '0x511517082f68c3604ef87f40bd98e3b1e37a54b7d4918c380a5c62e9e6f8c601',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1721512800000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x081a7840adef1fcba660166b380182083af6fe009f84821fb75f8105a2a60aa4',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1722722400000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x0cc8c2143be582d836c062e0b4ed54c478361e9454c33588daf04e948c24bc14',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1723932000000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0xa42903e420c9dac333be82300fcbc62edddcaa88da0ffa05b3a0351a01235571',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1725141600000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x5aff0e4a0dcc530ac5f6e74fa347ac618c8d4a72f0de9194ea3b967a67604189',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1726351200000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
            {
                timelockedStakedIotaId:
                    '0x33f5d6b77caa9dcef34bd6ed9b4f0510f225485e6bb54f992877fa85fd984cbd',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1727560800000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
        ],
    },
    {
        validatorAddress: '0x27838c3896b664b7dcc71a98f1dfda1fbbcb1383d60118da1d7fb938ebe4b8f1',
        stakingPool: '0xae37229d0e5779022b31b0ab9c539b02eb9c05659b2d59b3d7ce9c667ae1f3b1',
        stakes: [
            {
                timelockedStakedIotaId:
                    '0x305a9dd458d67124ffc61c8be38cd59c7d417b03184f1c23ba19b17e0d2d76d2',
                stakeRequestEpoch: '873',
                stakeActiveEpoch: '874',
                principal: '1000',
                status: 'Active',
                estimatedReward: '125059068',
                expirationTimestampMs: '1728770400000',
                label: SUPPLY_INCREASE_VESTING_LABEL,
            },
        ],
    },
];

export const mockedSupplyIncreaseVestingTimelockedObjectsWithDynamicDate =
    getMockedSupplyIncreaseVestingTimelockedObjectsWithDynamicDate(
        MOCKED_SUPPLY_INCREASE_VESTING_TIMELOCKED_OBJECTS,
    );

export const mockedVestingTimelockedStakedObjectsWithDynamicDate =
    getMockedVestingTimelockedStakedObjectsWithDynamicDate(
        MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS,
    );
