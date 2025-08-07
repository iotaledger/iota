// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Network } from '@iota/iota-sdk/client';

export type ValidatorAliasMap = Record<string, string>;

export const MAINNET_VALIDATOR_ALIASES: ValidatorAliasMap = {
    '0x864c651958094732a1227134cf7cab7587f05a399398804552553fbc01dba4e7': 'IOTA 1',
    '0x8e2e13c2ecfda356f07d008885b7bb82befb4d602245c3bff98ad59863162dd8': 'IOTA 2',
    '0xde58c0e225e01f890f05b0d2c19ae15b7c337faae539545bfd3a5d8676d62a87': 'IOTA 3',
};

export const TESTNET_VALIDATOR_ALIASES: ValidatorAliasMap = {
    '0x392316417a23198afeeb80d9fec314c65162ab5ad18f8a4c3375d31deab29670': 'IOTA Foundation 1',
    '0xa276b4c076fff55588255630e9ee35cf0d07e8d80c78991cfd58b43b687b4206': 'IOTA Foundation 2',
    '0xc571a9bb5da166b1de54ff90846399ddb63385c769e75cea4dce751e2fd29e55':
        'Tangle Ecosystem Association',
    '0xb64051fe5048486c0a215ff1ec48dc63214528bcc4d00c27d151404dbd717ba4':
        'IOTA Ecosystem DLT Foundation',
    '0x13ad84b8070dabba5a4cdb4b7714da2958829492385d4463f92a60956a0c24aa': 'iotalabs',
};

export const DEVNET_VALIDATOR_ALIASES: ValidatorAliasMap = {
    '0xda91b5957fe8e367b6c5d5fcbf48469f400a9395f959c35310703b2a78851afe': 'Validator 0',
    '0xbf73b3dc7a1e339e44f8441f20c7d4635d2b8c044c59cd46259b5c9152e68fc5': 'Validator 1',
    '0x446b7abcca53c58eba6720309bee8d5017ad10fde666f397bb9c04756c182597': 'Validator 2',
    '0x6f0202b12cd398166bdd3716c9aa3f0b6218ba125491f7ea2bc660fdd5e57ff8': 'Validator 3',
};

export const VALIDATOR_ALIASES_BY_NETWORK: {
    [key in Network]?: { enabled: boolean; addresses: Record<string, string> };
} = {
    [Network.Mainnet]: {
        enabled: true,
        addresses: MAINNET_VALIDATOR_ALIASES,
    },
    [Network.Testnet]: {
        enabled: true,
        addresses: TESTNET_VALIDATOR_ALIASES,
    },
    [Network.Devnet]: {
        enabled: true,
        addresses: DEVNET_VALIDATOR_ALIASES,
    },
};
