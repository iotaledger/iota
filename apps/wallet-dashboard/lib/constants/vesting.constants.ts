// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// https://github.com/iotaledger/iota/blob/1ec56b585905d7b96fb059a9f47135df6a82cd89/crates/iota-types/src/timelock/stardust_upgrade_label.rs#L12
const VESTING_LABEL =
    '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL';

interface ID {
    bytes: string;
}

interface UID {
    id: ID;
}

interface Balance {
    value: number;
}

interface Timelocked {
    id: UID;
    locked: Balance;
    expirationTimestampMs: number; // The epoch time stamp of when the lock expires
    label?: string;
}

interface StakedIota {
    id: UID;
    poolId: ID;
    stakeActivationEpoch: number;
    principal: Balance;
}

interface TimelockedStakedIota {
    id: UID;
    stakedIota: StakedIota;
    expirationTimestampMs: number;
    label?: string;
}

export const MOCKED_VESTING_TIMELOCKED_OBJECT: Timelocked[] = [
    {
        id: { id: { bytes: '0x2c035c185a81a737955bbea3902743e020f407e348a110e5615d6a7873aad5d8' } },
        locked: { value: 20 },
        expirationTimestampMs: 0,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x663c26b1db88872519baf4e60f22b1690b1a5dca8a16ff7b5602e08888c32612' } },
        locked: { value: 20 },
        expirationTimestampMs: 1209600000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x62a2145dc021dc68284272932660d8dd9750a0ddc3c9e05bbd7e8cf99e91a200' } },
        locked: { value: 20 },
        expirationTimestampMs: 2419200000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x02e06822806080912724e175efcc8507f83dd4500246ec3fec5de263d37587d9' } },
        locked: { value: 20 },
        expirationTimestampMs: 3628800000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0xaf9f2d6bb9bcdf7b0341c418289628b6db222413baa0948b4299b0754360b614' } },
        locked: { value: 20 },
        expirationTimestampMs: 4838400000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0xd4d97f3d7582a3a28c6833dffa6405de986c1e984ef5f291b39daf3e6501c940' } },
        locked: { value: 20 },
        expirationTimestampMs: 6048000000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0xa1ca39faa8099f57ed39529095551dccb456c29f36e312111d146849078c7984' } },
        locked: { value: 20 },
        expirationTimestampMs: 7257600000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x0d0a4009b389da15132cc244d70453d3e15e0548d192b76fd5c36ec95068654e' } },
        locked: { value: 20 },
        expirationTimestampMs: 8467200000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x4d6fe5ad14a20cba8fc5865b28762a586603e4ac501b0b40516140bb0279d984' } },
        locked: { value: 20 },
        expirationTimestampMs: 9676800000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x0d5f720c0d6bcb6ce797c91394b91c4320f10d5f342c68ef4636544547635766' } },
        locked: { value: 20 },
        expirationTimestampMs: 10886400000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x16ed3f52bb949bbd609efc76401135faf398ba93f89fc164ce3afb9956460563' } },
        locked: { value: 20 },
        expirationTimestampMs: 12096000000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0xe430875bcadc4b938147060bd6382c44728f2e64addd13a694b8e72cf3c1e40c' } },
        locked: { value: 20 },
        expirationTimestampMs: 13305600000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x34c31d595c5043577290ffc0e09e00ac29bb2680e393e2c4ff8b2dd9f42f3765' } },
        locked: { value: 20 },
        expirationTimestampMs: 14515200000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x1fd72de4dfd037e90350586e74711283ea741d0c3cd54650ffb24e0770ce9221' } },
        locked: { value: 20 },
        expirationTimestampMs: 15724800000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0xb154436669323b0c70725dad92d36b4f5b873dbc7d74fc3e2311021d5e3cf4d6' } },
        locked: { value: 20 },
        expirationTimestampMs: 16934400000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x66881519deab5061ae27f8d7a8b683e463fb20085cb27066124a12ae6db7a59c' } },
        locked: { value: 20 },
        expirationTimestampMs: 18144000000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x5a4506276e53143f2583454f4021101491a2990d4151bb8172a8ed868065775d' } },
        locked: { value: 20 },
        expirationTimestampMs: 19353600000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x872eff6831ce7cf9c30a4d34be1c480c608861926a6e3f78b37ba12a6eb0c213' } },
        locked: { value: 20 },
        expirationTimestampMs: 20563200000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x1f1d23aeec7eecc5cbca78c38dc1a5d17b623320eb6329c066b4aad74b0a09f3' } },
        locked: { value: 20 },
        expirationTimestampMs: 21772800000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x7a20b0307152f54c5c7dff0daff97ac9d448ec267984efffb810da7f8fde015e' } },
        locked: { value: 20 },
        expirationTimestampMs: 22982400000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x73a3f986c0dc9ea45150109375d4226fdf8014e962e8fbcea97e139fa1773203' } },
        locked: { value: 20 },
        expirationTimestampMs: 24192000000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x409788b337a1afa86baaeaf39c0902de1d6886447a129491d37bdb83531261ea' } },
        locked: { value: 20 },
        expirationTimestampMs: 25401600000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x2f90385d39ef100ec79a0e8914232a7561f2256a6100104d75d1345ba80333d7' } },
        locked: { value: 20 },
        expirationTimestampMs: 26611200000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x7884c52d2b9cd2904a9c034408e05c7230252bb90ec8030350fd63912e2cc5d1' } },
        locked: { value: 20 },
        expirationTimestampMs: 27820800000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0xdf76a70092f29455884833eb755225c887ad92160c10746bf641f11a1c879181' } },
        locked: { value: 20 },
        expirationTimestampMs: 29030400000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x4c3a644dbba3b99f422229fac1b4ee704a05722e4eb9f0f1d692128c03b3cd19' } },
        locked: { value: 20 },
        expirationTimestampMs: 30240000000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0xcfc174eb90ce203c615ae6c2aaea54156dd731a7dba5b248ccadda7ab8731bba' } },
        locked: { value: 20 },
        expirationTimestampMs: 31449600000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x532d934d4ff41c8ddab15b5972308cccb1b174ec24face2d05e607c885ba2a5b' } },
        locked: { value: 20 },
        expirationTimestampMs: 32659200000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x60564b231289c88fe92737c4384902a5864769c805fa504597682ae1afc609d0' } },
        locked: { value: 20 },
        expirationTimestampMs: 33868800000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0xfc5596072dbd1297f115f9c53819ed9d775dc3fdffcf2bad1481e6e00fc9bead' } },
        locked: { value: 20 },
        expirationTimestampMs: 35078400000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x0339d660a1b2d9213cb7bb4e82d91d7550ae530e775480260330f524868953f4' } },
        locked: { value: 20 },
        expirationTimestampMs: 36288000000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0xbb506402dfd6aa0e8cf7fb26ba51f10a0c0895d338a9c545d106d9905bacf3a3' } },
        locked: { value: 20 },
        expirationTimestampMs: 37497600000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
    {
        id: { id: { bytes: '0x1ecbaceb438d85bd40218903c1316c30d055c912a71e7fa4e216bd8b862d0b6c' } },
        locked: { value: 20 },
        expirationTimestampMs: 38707200000,
        label: '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL',
    },
];

export const MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS: TimelockedStakedIota[] = [
    {
        id: { id: { bytes: '0xd252a449e46df3a78a94982f8bec2a9a7ab6251d0852942fd54d438b888756ee' } },
        stakedIota: {
            id: {
                id: { bytes: '0x4846a1f1030deffd9dea59016402d832588cf7e0c27b9e4c1a63d2b5e152873a' },
            },
            poolId: { bytes: '0xaeab97f96cf9877fee2883315d459552b2b921edc16d7ceac6eab944dd88919c' },
            stakeActivationEpoch: 1000,
            principal: { value: 100 },
        },
        expirationTimestampMs: 1720091758000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x1f9310238ee9298fb703c3419030b35b22bb1cc37113e3bb5007c99aec79e5b8' } },
        stakedIota: {
            id: {
                id: { bytes: '0x8267a85a21bb527ad4545bc29452cca715d3a1aa8975e4ef1e77f9862c9a9244' },
            },
            poolId: { bytes: '0x4216768d1e645dd1c0ee15f118b99935362adecfaf305aeb13690f14105be158' },
            stakeActivationEpoch: 1010,
            principal: { value: 250 },
        },
        expirationTimestampMs: 1720092138000,
        label: VESTING_LABEL,
    },
];

export const MOCKED_VESTING_TIMELOCKED_AND_TIMELOCK_STAKED_OBJECTS: (
    | Timelocked
    | TimelockedStakedIota
)[] = [...MOCKED_VESTING_TIMELOCKED_OBJECT, ...MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS];
