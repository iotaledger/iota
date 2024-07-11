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
    expirationTimestampMs: number;
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
        id: { id: { bytes: '0xc455b0619ed8f1e6b1fee1176a5f8ea36f8494b9bd5f1f17ec5ac81f1d8b2e51' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1720051200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x0727f69495a6956159c669044647b63f35bbe366d9160ef65ea98d47f3984f5e' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1721260800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x1730a0abd4dcb3a6ebe7b1a7e40ae27d79a5bc697777c2457ce3cd940e9e8a89' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1722470400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x7dc2ac1145773a878e6f3267b90a98d0941dd626ccf8cdd5754cf2abdba9b779' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1723680000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xbdf74cf6e26166b700d0425745804d51d686452f3cf5ec29e9a94a4bc4d3714b' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1724889600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x47c03391afeb4256f9ff3750e9edfde205e561b6e1bd702ae1defe26eb45a6ff' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1726099200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xc4116461424ca5ab4ce68978ead301d71cb938a3c65430ac2b3bb742c9dca3e3' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1727308800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x9b86b857ba65b9b7225d7b73f6d0725ba446a27b2f20c965dc15d518b840638a' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1728518400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x340d11b6aa921aa18aba0de6be934144a9601df221b7176df160d74ab44531da' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1729728000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x811cda705c194f1aece35307deb82b7e0e96b0993f80e59acb194a04eaa41c76' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1730937600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x1eefe64258896d45d806561a4d100bc11afcc10fcb4eb49110fe1065fd203977' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1732147200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xd6a725f503c89f245d25cec129d4bebf5c797ead02fc445b2140fbc4b310f6a9' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1733356800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x02c8f573ff4d3f5a65e0ed0b51c563d90728760205124f267165ef2f9c97d260' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1734566400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x6816d00e50400d5e091006daa969d189342c732e11d23b52d8216bd5accbb097' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1735776000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x60bcec283b79b5a2039460d9f41e4aedd285b725fb440454876957f908d3ff3a' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1736985600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xcd5b2e2960d2efeeb61f85b4fb95f3bf8d693de4e4959f2d600d2351aa9c7fbe' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1738195200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x641426994cc0974c435ba0ae81a6dc6f2ae7eec38868fb4def9b55435ac2634b' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1739404800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x6079d52997f1a392837acfc46fe9a0c4891a3535017438fe90de20dad17d1069' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1740614400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x7a0796bd49dc6188627743669b1b6a906f3ddf6666550208e4c1c5b03e11af2f' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1741824000000,
        label: VESTING_LABEL,
    },
    {
        id: {
            id: { bytes: '0Closef6152d586e9456a1a3cc7d3d0c9def8352850d06f9284f1b234b63345dd18b' },
        },
        locked: { value: 1000 },
        expirationTimestampMs: 1743033600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xda1996c0f9997374fbb8394ae0e54e4aca1dd9708c92af9af360503b8263a3d4' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1744243200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xee4f8cf39d4c93116f3559695454ff1c230482041da3b3b734e2703d3a5b350b' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1745452800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xe84ae3f1548e9cdbaedf1d047099bcbce270eec6d5d8fc9d9e8e97c22ea1a644' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1746662400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xd622766d1af862cb3aaea928bb2847bd5ccb3ff38175136ec546f9e2a9035d44' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1747872000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x3b4f873dcc8eca630f922b0903e785811deda24faee75b7de3767124d3f639e8' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1749081600000,
        label: VESTING_LABEL,
    },
];

export const MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS: TimelockedStakedIota[] = [
    {
        id: { id: { bytes: '0x1d981f9fde96c2e509de1c925a95b78b2a3cb910d9b384ca4dbeb1bd14aa1cf2' } },
        stakedIota: {
            id: {
                id: { bytes: '0x2a1df8ec18ef82da39f8af22ae1b8656037706df377ad4af0fc2036f50373f1d' },
            },
            poolId: { bytes: '0xea1a6f7ff4c03ce2a56687716d9c6e373286d2dca12cc0a4a86c2b943173553c' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1720051200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x72b7bafbe81584599b8c8d1e58758fd6f34e4a4e65fe22899cf4485063826aee' } },
        stakedIota: {
            id: {
                id: { bytes: '0x75e69abfc76ad38944e747f36ecf0dfd0933f80134187c7a67952f0011623b21' },
            },
            poolId: { bytes: '0xddd255ac76d01579d2d873cc0b0548ad58a11c18ac41c75c03aa0339890ef6ac' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1721260800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xe438122ed11983492bfcabdd78b52d0739124802f8667fefcbdea4d0e1f6ff55' } },
        stakedIota: {
            id: {
                id: { bytes: '0x6c9abc5d279d79f1693f09fa220300ef8483bcbdcca410e3d533e4892d7a60f9' },
            },
            poolId: { bytes: '0xb6930369f558843ff3f4d49edfef0e80d526efba150d0772b35d3990df5b4531' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1722470400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x6f1f54bda98e0d82cdb90045fee0bb32bae8672f19e4c7797cb28409898c9a3f' } },
        stakedIota: {
            id: {
                id: { bytes: '0x40d2fcada5c4b87854b458115d678d87317bf14b28abce0ae94be2063a5c9c0f' },
            },
            poolId: { bytes: '0xea1a6f7ff4c03ce2a56687716d9c6e373286d2dca12cc0a4a86c2b943173553c' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1723680000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xfb476568cd1e6563874a2a325677be253a3dfe46872c9ce89eb8af3ea731dea6' } },
        stakedIota: {
            id: {
                id: { bytes: '0xfa414fb4078c7424f353a0206a0d18a07a21ff5ccc99e81fd15cf201fd0a65d4' },
            },
            poolId: { bytes: '0xb6930369f558843ff3f4d49edfef0e80d526efba150d0772b35d3990df5b4531' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1724889600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x323760b2fea142c255ec9fb7c75a2380adb1c41cd65ca704e7076564f9db990c' } },
        stakedIota: {
            id: {
                id: { bytes: '0x67564a23c8a07f02755c8f23d3d97ed23de5f1af1b702e23e0fe6d5b68592334' },
            },
            poolId: { bytes: '0xb6930369f558843ff3f4d49edfef0e80d526efba150d0772b35d3990df5b4531' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1726099200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xc556e3e84b39f730d6fd7ea152d2f947526b45c989a03633e5a79186fe52a3a0' } },
        stakedIota: {
            id: {
                id: { bytes: '0xd2f93f458c41ace2099f877f97233fc84f04eafbfb5a48b39ef15896bf34dcdb' },
            },
            poolId: { bytes: '0xb6930369f558843ff3f4d49edfef0e80d526efba150d0772b35d3990df5b4531' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1727308800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x2b771bd4c3b0f36a261ff1249d8bf70858e5c53ca3182c8088ea53e0e62d9ba3' } },
        stakedIota: {
            id: {
                id: { bytes: '0x38847bb6e80fc93d2a1924e65f37aa7f39c2b66c9cd0465cba4f8f7a2aa69cf4' },
            },
            poolId: { bytes: '0xea1a6f7ff4c03ce2a56687716d9c6e373286d2dca12cc0a4a86c2b943173553c' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1728518400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x97d108f7ead86885654018931854c75314d82ee25d1bd3e25e169bd9ef848965' } },
        stakedIota: {
            id: {
                id: { bytes: '0xe9c230f1046e3460d38ff70c46aa7e4b812d797e81d91e6048966dd457516908' },
            },
            poolId: { bytes: '0xea1a6f7ff4c03ce2a56687716d9c6e373286d2dca12cc0a4a86c2b943173553c' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1729728000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x72793b91fea132f81df2065cd78e597a0b426da1b75b737689529fbaa7ae5e02' } },
        stakedIota: {
            id: {
                id: { bytes: '0x584049fcd0854e2d9ae7f5442ddcd7a6774941cea32f446baa42ed471c0c9b5e' },
            },
            poolId: { bytes: '0xddd255ac76d01579d2d873cc0b0548ad58a11c18ac41c75c03aa0339890ef6ac' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1730937600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xc7a986cbdaf4d7b6f5167b2706f3f2d692846fee010f55f46540987a81a5a0d9' } },
        stakedIota: {
            id: {
                id: { bytes: '0x0fce2e04c142b904eddd7b644161335387e0c76398add8d4b75af8b973eb06c1' },
            },
            poolId: { bytes: '0xae37229d0e5779022b31b0ab9c539b02eb9c05659b2d59b3d7ce9c667ae1f3b1' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1732147200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xac1a96522df60536fd5bb6e0ad9452870b623262cedb01bc28eedd5322d849d2' } },
        stakedIota: {
            id: {
                id: { bytes: '0x8b3145f22980a2d506aa4d179657ca3acf2196509b4982d06d6c6a1cc033d47c' },
            },
            poolId: { bytes: '0xb6930369f558843ff3f4d49edfef0e80d526efba150d0772b35d3990df5b4531' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1733356800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x69adfd0c384f62e1d56b4658521b84c3343418187fb3b53fd8836ec20c294477' } },
        stakedIota: {
            id: {
                id: { bytes: '0x687a70de2a11071592da1e1c7e65530407577974a253fbb291ab694fa1862556' },
            },
            poolId: { bytes: '0xddd255ac76d01579d2d873cc0b0548ad58a11c18ac41c75c03aa0339890ef6ac' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1734566400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x2a60bbc881f361455155158dd28bbf70bf532f775d9e397e98629c338b254354' } },
        stakedIota: {
            id: {
                id: { bytes: '0xcb5b7b159752b1854c368d2f178f92e579a27c64a882669ca8ffddb921d5934e' },
            },
            poolId: { bytes: '0xea1a6f7ff4c03ce2a56687716d9c6e373286d2dca12cc0a4a86c2b943173553c' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1735776000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x50468640fc1c7623bd380ed93e6e6e7a0578c26dd0d78a8a0894c5cfd3718162' } },
        stakedIota: {
            id: {
                id: { bytes: '0x9d44592883927293bd177924cca351bc7c6c075a834d44711bb5f85cebd47cb9' },
            },
            poolId: { bytes: '0xddd255ac76d01579d2d873cc0b0548ad58a11c18ac41c75c03aa0339890ef6ac' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1736985600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xb220dfabba1985b2d3f4ef4899561b12c3d921aab8607cce87e02a7bfaa7c7ca' } },
        stakedIota: {
            id: {
                id: { bytes: '0x72b8c7e5695e86b043b4f66202687fe7d1a11a0118bc795133f2af2f5229b4c7' },
            },
            poolId: { bytes: '0xddd255ac76d01579d2d873cc0b0548ad58a11c18ac41c75c03aa0339890ef6ac' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1738195200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x303fc97a8b1c20fed2c0732b8ad6290725dcb668a2224f3e79cbbe28c7c1cde6' } },
        stakedIota: {
            id: {
                id: { bytes: '0x7bbd702f91697c81e05a17c6c6cca7160032627e1ca3af2736fd826f42196ff7' },
            },
            poolId: { bytes: '0xea1a6f7ff4c03ce2a56687716d9c6e373286d2dca12cc0a4a86c2b943173553c' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1739404800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x81c7661212e17f6acd3620a4f4191b350a350b6fdabdefdd7f0940b962f5e6e3' } },
        stakedIota: {
            id: {
                id: { bytes: '0xaf4544a2086985b0c29fee18df4f2fe616370824ef6d62c2965615c74f53fbee' },
            },
            poolId: { bytes: '0xea1a6f7ff4c03ce2a56687716d9c6e373286d2dca12cc0a4a86c2b943173553c' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1740614400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x4a20ce4aec17d1ec33fcb305195422b0ef4dd1410c267f79e0cb80c4f9232fe0' } },
        stakedIota: {
            id: {
                id: { bytes: '0x511517082f68c3604ef87f40bd98e3b1e37a54b7d4918c380a5c62e9e6f8c601' },
            },
            poolId: { bytes: '0xea1a6f7ff4c03ce2a56687716d9c6e373286d2dca12cc0a4a86c2b943173553c' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1741824000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x4b6ee8ee6dcc2eeeea0861ce658c4e4e684c80dba901442ed5ef69addd8a45d6' } },
        stakedIota: {
            id: {
                id: { bytes: '0x081a7840adef1fcba660166b380182083af6fe009f84821fb75f8105a2a60aa4' },
            },
            poolId: { bytes: '0xea1a6f7ff4c03ce2a56687716d9c6e373286d2dca12cc0a4a86c2b943173553c' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1743033600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x429b75bcad49db078fcbb9d23e64b2596429c3657f48c84e12e66ea4c8c0e3a7' } },
        stakedIota: {
            id: {
                id: { bytes: '0x0cc8c2143be582d836c062e0b4ed54c478361e9454c33588daf04e948c24bc14' },
            },
            poolId: { bytes: '0xb6930369f558843ff3f4d49edfef0e80d526efba150d0772b35d3990df5b4531' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1744243200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xab5d982443f648472dd8a2d06fb760067267b2d8c08a4d7c6e7464eb58dac832' } },
        stakedIota: {
            id: {
                id: { bytes: '0xa42903e420c9dac333be82300fcbc62edddcaa88da0ffa05b3a0351a01235571' },
            },
            poolId: { bytes: '0xb6930369f558843ff3f4d49edfef0e80d526efba150d0772b35d3990df5b4531' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1745452800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xd2cfd53537bff38e4ce90db46cbd15fa76f766a1c2cdc5aa2a075ae0e3ed2b8b' } },
        stakedIota: {
            id: {
                id: { bytes: '0x5aff0e4a0dcc530ac5f6e74fa347ac618c8d4a72f0de9194ea3b967a67604189' },
            },
            poolId: { bytes: '0xae37229d0e5779022b31b0ab9c539b02eb9c05659b2d59b3d7ce9c667ae1f3b1' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1746662400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x3be9dcf0bc43471220a4529c206191b213244ac2f0e16ac40df41cca3ce98122' } },
        stakedIota: {
            id: {
                id: { bytes: '0x33f5d6b77caa9dcef34bd6ed9b4f0510f225485e6bb54f992877fa85fd984cbd' },
            },
            poolId: { bytes: '0xddd255ac76d01579d2d873cc0b0548ad58a11c18ac41c75c03aa0339890ef6ac' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1747872000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x27838c3896b664b7dcc71a98f1dfda1fbbcb1383d60118da1d7fb938ebe4b8f1' } },
        stakedIota: {
            id: {
                id: { bytes: '0x305a9dd458d67124ffc61c8be38cd59c7d417b03184f1c23ba19b17e0d2d76d2' },
            },
            poolId: { bytes: '0xae37229d0e5779022b31b0ab9c539b02eb9c05659b2d59b3d7ce9c667ae1f3b1' },
            stakeActivationEpoch: 5555,
            principal: { value: 1000 },
        },
        expirationTimestampMs: 1749081600000,
        label: VESTING_LABEL,
    },
];

export const MOCKED_VESTING_TIMELOCKED_AND_TIMELOCK_STAKED_OBJECTS: (
    | Timelocked
    | TimelockedStakedIota
)[] = [
    {
        id: { id: { bytes: '0x286b2c0317cc9dcfb8cc571c5f6b92c2f69b4d9666580b454291b0946fc3b37b' } },
        locked: { value: 589 },
        expirationTimestampMs: 1720051200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xe574e0d2abb35b96f8b3918f5a1fdb66e71733fd7c02dee71abdd5427e9b3431' } },
        stakedIota: {
            id: {
                id: { bytes: '0xb417430c07e03845e58fdde3f187acda86aea1e59c2c1dabd0d1d676b4845a75' },
            },
            poolId: { bytes: '0x2a0abc81a99e44e3dc9ec656f56cc99e1fdbd76137dd882116ab7281298df6fc' },
            stakeActivationEpoch: 5555,
            principal: { value: 411 },
        },
        expirationTimestampMs: 1720051200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x38bf384d2dc4b9a5c2584432db3787d3e911231f7115a3b31fa9fd5652220e76' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1721260800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xa4c61f4c58fc899a058916bbbd0e9215b1a88c04c360ca92db50ab313974f6b6' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1722470400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x972452318c6bcf35b6a73757866c66af64e6b0284a3cac676ec70ad7c2c6cabe' } },
        locked: { value: 144 },
        expirationTimestampMs: 1723680000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x55fb53235e7f7dea2239f4d5e69c8fc85f35e25fac6b76500f51f2a3ea181532' } },
        stakedIota: {
            id: {
                id: { bytes: '0x7050c4d8a28eb65043c802f603e8c3c5b9af4ad5d061b78c68c914e8b2dfc01e' },
            },
            poolId: { bytes: '0x2a0abc81a99e44e3dc9ec656f56cc99e1fdbd76137dd882116ab7281298df6fc' },
            stakeActivationEpoch: 5555,
            principal: { value: 856 },
        },
        expirationTimestampMs: 1723680000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x8be689ee351c1f455e12ea67fe21caf3989a8b029dfe79fa5bf0e47d2eac0f6a' } },
        locked: { value: 7 },
        expirationTimestampMs: 1724889600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x09d4bdbd96e33f5c1e1666302483a967d3d4307048c7620512bcdafbc09f6abc' } },
        stakedIota: {
            id: {
                id: { bytes: '0x6eec46a3cae99c4398df87cf3e6ebeb3893e4932e6a5dfa69cfdfe7d842258a0' },
            },
            poolId: { bytes: '0x7f61357fa975841cacaa9a5776ea84a8ec8d7d5a7daa0dc7fa8a7c236482af1d' },
            stakeActivationEpoch: 5555,
            principal: { value: 993 },
        },
        expirationTimestampMs: 1724889600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xd77236288ee66f6e8d677a974efe7a0b0b5c7e6d1154da7601d9a95560186965' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1726099200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x99fb87d3a7b37bbe08d2634347ecebf11583e840e6a35c58d440b5ff3ab3c7c1' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1727308800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xcc26159c849a70969fc4b8aa4b3f7caabd8fcad107e6523ddfac6df27edea921' } },
        locked: { value: 494 },
        expirationTimestampMs: 1728518400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x4722f43ca6088455583c59051d1831f8ba9749c0ae0fc4e476e7a3362c9db928' } },
        stakedIota: {
            id: {
                id: { bytes: '0xdfd86394269a34a4418fcf2095994fb2952bdc59c483e9f7e43f1dc638d28ae7' },
            },
            poolId: { bytes: '0x7f61357fa975841cacaa9a5776ea84a8ec8d7d5a7daa0dc7fa8a7c236482af1d' },
            stakeActivationEpoch: 5555,
            principal: { value: 506 },
        },
        expirationTimestampMs: 1728518400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x479517cc209dc504575993ae9a1691e0a73301d98d63100ea8e5c253c4f79525' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1729728000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x16f4cd14e15efc95a35c9bd5e8da118fbc144aa77f1c63387e2ea438736af3fe' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1730937600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xa0924d0c92513ece94b3aa1e827e76d713c8625c1e6c8f0011be6ecfa3df3e76' } },
        locked: { value: 711 },
        expirationTimestampMs: 1732147200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x8670f828d3b82be680edee8f8cf9823ca9485e54f8728487d0aa0d367f389ab1' } },
        stakedIota: {
            id: {
                id: { bytes: '0xff806527381322c9d016230b9d75f5638346aab72e1f5e291b3eb5499122934c' },
            },
            poolId: { bytes: '0x7f61357fa975841cacaa9a5776ea84a8ec8d7d5a7daa0dc7fa8a7c236482af1d' },
            stakeActivationEpoch: 5555,
            principal: { value: 289 },
        },
        expirationTimestampMs: 1732147200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x46e385d485df4581ae373c1ce623b4f513bf0c35ee2cc9d10f8410a3b4ee2cb6' } },
        locked: { value: 130 },
        expirationTimestampMs: 1733356800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x197ac9f7b5d0acd93159083ec755916ce3e5ade5cbedf7a49107c8603808069f' } },
        stakedIota: {
            id: {
                id: { bytes: '0x69d6a5af46a3bf0bce5a97d6a5c5a120293afe2031df08b787c1e0c89eef54e3' },
            },
            poolId: { bytes: '0xcd707a1c55ced6a8ed6d3fc1893a4578b5d840e4555200bf154978619b5599a8' },
            stakeActivationEpoch: 5555,
            principal: { value: 870 },
        },
        expirationTimestampMs: 1733356800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x7c056b2af944745852e505086cc793b72e3325ccf7c3c71ec0544f7d49925621' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1734566400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xac214e255e243a7e1aceef94ae7709f7200f7fccbefef67497c59aac4b95b6f8' } },
        locked: { value: 868 },
        expirationTimestampMs: 1735776000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xdce49e0aae09e8643dc997a99bfa7a927247fa2be4de350a970244a8d91ff672' } },
        stakedIota: {
            id: {
                id: { bytes: '0x8d1fa6d30174054a212edf409026a4c9427987361103f7b74797515f860ed763' },
            },
            poolId: { bytes: '0x2a0abc81a99e44e3dc9ec656f56cc99e1fdbd76137dd882116ab7281298df6fc' },
            stakeActivationEpoch: 5555,
            principal: { value: 132 },
        },
        expirationTimestampMs: 1735776000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x92bcfa0735940ff8682abe71b0d431712a3ff921b94f1c43b21f51c124e17efd' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1736985600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x15a8796a100bbb71a2fe95d987b3b961f1980293673ea19a320d42ae0dd557c6' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1738195200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x3ddcf7812912bdf7d1c7d015ea418c0734698c6ff5fa0a6d737103709539effc' } },
        locked: { value: 103 },
        expirationTimestampMs: 1739404800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x1cb238a7c0dddf9ffae6857821518397d68662826ee8361de16d6caece41f1ab' } },
        stakedIota: {
            id: {
                id: { bytes: '0x69dfc02157f9033d71677586fbb78c091a9c885dc93cc6f3b312c0aee94c64d3' },
            },
            poolId: { bytes: '0x5f71be4051e4c0dc71e97c35e6b76228b3b5ef081eea6ac94eef895c461754aa' },
            stakeActivationEpoch: 5555,
            principal: { value: 897 },
        },
        expirationTimestampMs: 1739404800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x84960f357289ecf9b81ced44bc5d80b1868b6d3816183a3b56b8690425a4bfa3' } },
        locked: { value: 299 },
        expirationTimestampMs: 1740614400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x53f54d864faa6287d17a53b806bc3f2127d8b32e62303dd6d729c2809f715336' } },
        stakedIota: {
            id: {
                id: { bytes: '0xdc88e1f7c5323a0f7009a1cde781e4e61a375400e7a390c88886d8cd55169d5d' },
            },
            poolId: { bytes: '0x7f61357fa975841cacaa9a5776ea84a8ec8d7d5a7daa0dc7fa8a7c236482af1d' },
            stakeActivationEpoch: 5555,
            principal: { value: 701 },
        },
        expirationTimestampMs: 1740614400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xba31794b7a1dbf11975b08e571fa07346b546678e8460f88d87d55ddba040478' } },
        locked: { value: 500 },
        expirationTimestampMs: 1741824000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x17693a56211258fcd5a994a007704fdd9967b52851451b46d6ba330812a5cc3f' } },
        stakedIota: {
            id: {
                id: { bytes: '0xbe72eb343db6325370586436a4dfc5e248bb009fd93651fc574da4879ee7041d' },
            },
            poolId: { bytes: '0xcd707a1c55ced6a8ed6d3fc1893a4578b5d840e4555200bf154978619b5599a8' },
            stakeActivationEpoch: 5555,
            principal: { value: 500 },
        },
        expirationTimestampMs: 1741824000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x10e6438bff39079e4518dcb1a87ba95ca65c287b593a0bf9079dabdf77d6233d' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1743033600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x8ab5eff4f36b1ba0d99624ce8285362784e9bcb4ab18e7553ed9f74d1d521fce' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1744243200000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x72d7f86677cf4fcc7397a9f62af722a7290e072150f457f60d0548d7b712c22c' } },
        locked: { value: 1000 },
        expirationTimestampMs: 1745452800000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x1672412d036ad1610f8f8c39b71babfd0a0f0a03d3c8b637635d327842132acd' } },
        locked: { value: 347 },
        expirationTimestampMs: 1746662400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x8327bbe04ac8dc866d6f18c98a20ee3137c26ccc72b62effca73402e055a57d7' } },
        stakedIota: {
            id: {
                id: { bytes: '0x993b8db7d52e20e24fee279d0effd380f9731c1d07bea6abac545357ca73d316' },
            },
            poolId: { bytes: '0x2a0abc81a99e44e3dc9ec656f56cc99e1fdbd76137dd882116ab7281298df6fc' },
            stakeActivationEpoch: 5555,
            principal: { value: 653 },
        },
        expirationTimestampMs: 1746662400000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x1de6fd9da4de9ac869f728060b659c9fb170669213c476adc6763a03c25809c5' } },
        locked: { value: 792 },
        expirationTimestampMs: 1747872000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xd7ad38945ecb7ca6eeaadbf321703b263b4f6ef714e211d00b2aa8fe241fce0c' } },
        stakedIota: {
            id: {
                id: { bytes: '0x850dc90cacec45a555a05f9be3c1b3a828d43bfd7fcaf7c8789319d1369832af' },
            },
            poolId: { bytes: '0xcd707a1c55ced6a8ed6d3fc1893a4578b5d840e4555200bf154978619b5599a8' },
            stakeActivationEpoch: 5555,
            principal: { value: 208 },
        },
        expirationTimestampMs: 1747872000000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x900469a81978d06246234dcf508ff1180b1521184373b87b40c83ae42466b2dc' } },
        locked: { value: 49 },
        expirationTimestampMs: 1749081600000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xfae93493826f45ac789f3319316aa55f1a707c171c9edfae5c12b845713163d1' } },
        stakedIota: {
            id: {
                id: { bytes: '0xd194fa4d9a909baec026d8d67e33a85a9d36921df7a47288ed3e8a5eae9b8b9c' },
            },
            poolId: { bytes: '0x5f71be4051e4c0dc71e97c35e6b76228b3b5ef081eea6ac94eef895c461754aa' },
            stakeActivationEpoch: 5555,
            principal: { value: 951 },
        },
        expirationTimestampMs: 1749081600000,
        label: VESTING_LABEL,
    },
];
