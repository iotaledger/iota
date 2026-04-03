// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, it, expect } from 'vitest';
import { createUnstakeValidationSchema } from '../createUnstakeValidationSchema';

const IOTA_DECIMALS = 9;
const MIN_STAKE = 1_000_000_000n; // 1 IOTA
const PRINCIPAL = 10_000_000_000n; // 10 IOTA
const COIN_SYMBOL = 'IOTA';

function createSchema(principalAmount = PRINCIPAL) {
    return createUnstakeValidationSchema(principalAmount, COIN_SYMBOL, IOTA_DECIMALS, MIN_STAKE);
}

async function validateAmount(amount: string, principalAmount?: bigint) {
    const schema = createSchema(principalAmount);
    try {
        await schema.validate({ amount });
        return null;
    } catch (err: unknown) {
        return (err as { message: string }).message;
    }
}

describe('createUnstakeValidationSchema', () => {
    it('should accept a valid unstake amount', async () => {
        const error = await validateAmount('5');
        expect(error).toBeNull();
    });

    it('should reject empty amount', async () => {
        const error = await validateAmount('');
        expect(error).not.toBeNull();
    });

    it('should reject non-numeric input', async () => {
        const error = await validateAmount('abc');
        expect(error).toContain('not valid');
    });

    it('should reject amount below minimum stake (1 IOTA)', async () => {
        const error = await validateAmount('0.5');
        expect(error).toContain('at least 1');
    });

    it('should accept exactly 1 IOTA (minimum)', async () => {
        const error = await validateAmount('1');
        expect(error).toBeNull();
    });

    it('should reject amount exceeding principal', async () => {
        const error = await validateAmount('15');
        expect(error).toContain('must be less than');
    });

    it('should reject amount that leaves remaining below minimum', async () => {
        // 10 IOTA principal, unstake 9.5 leaves 0.5 IOTA remaining
        const error = await validateAmount('9.5');
        expect(error).toContain('Remaining stake must be at least 1');
    });

    it('should accept amount that leaves exactly 1 IOTA remaining', async () => {
        // 10 IOTA principal, unstake 9 leaves 1 IOTA remaining
        const error = await validateAmount('9');
        expect(error).toBeNull();
    });

    it('should accept full unstake (remaining = 0)', async () => {
        // 10 IOTA principal, unstake 10 leaves 0 remaining (full unstake)
        const error = await validateAmount('10');
        expect(error).toBeNull();
    });

    it('should reject excessive decimals', async () => {
        const error = await validateAmount('1.12345678901');
        expect(error).toContain('maximum decimals');
    });

    it('should accept amount with valid decimal precision', async () => {
        const error = await validateAmount('1.123456789');
        expect(error).toBeNull();
    });
});
