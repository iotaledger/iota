// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';
import { formatType } from '../../src/utils';

describe('Formatters', () => {
    it('formatType formats correctly', async () => {
        const typeClock = '0x2::clock::Clock';
        expect(formatType(typeClock), '0x0000…0002::clock::Clock');

        const typeCoolCoin =
            '0x2::coin::Coin<0x2e0b8d1e74947a2d97121bc7b7981eaff1f32911d15c5e0921fdd08cf61f445b::cool_coin::COOL_COIN>';
        expect(formatType(typeCoolCoin), '0x0000…0002::coin::Coin<x2e0b8d…445b::cool_coin::COOL_COIN>');
    });
});
