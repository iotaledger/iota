// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

vi.doMock('../../../wallet-standard/src/chains.ts', () => ({
    isSupportedChain: (id: string) => id.split(':')[0] === 'iota',
}));
