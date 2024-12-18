// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import BigNumber from 'bignumber.js';
import { describe, expect, it } from 'vitest';

import { CoinFormat, formatBalance } from '../useFormatCoin';

const IOTA_DECIMALS = 9;

function toNanos(iota: string) {
	return new BigNumber(iota).shiftedBy(IOTA_DECIMALS).toString();
}

describe('formatBalance', () => {
	it('formats zero amounts correctly', () => {
		expect(formatBalance('0', 0)).toEqual('0');
		expect(formatBalance('0', IOTA_DECIMALS)).toEqual('0');
	});

	it('formats decimal amounts correctly', () => {
		expect(formatBalance('0', IOTA_DECIMALS)).toEqual('0');
		expect(formatBalance('0.000', IOTA_DECIMALS)).toEqual('0');
	});

	it('formats integer amounts correctly', () => {
		expect(formatBalance(toNanos('1'), IOTA_DECIMALS)).toEqual('1');
		expect(formatBalance(toNanos('1.0001'), IOTA_DECIMALS)).toEqual('1');
		expect(formatBalance(toNanos('1.1201'), IOTA_DECIMALS)).toEqual('1.12');
		expect(formatBalance(toNanos('1.1234'), IOTA_DECIMALS)).toEqual('1.12');
		expect(formatBalance(toNanos('1.1239'), IOTA_DECIMALS)).toEqual('1.12');

		expect(formatBalance(toNanos('9999.9999'), IOTA_DECIMALS)).toEqual('9,999.99');
		// 10k + handling:
		expect(formatBalance(toNanos('10000'), IOTA_DECIMALS)).toEqual('10 K');
		expect(formatBalance(toNanos('12345'), IOTA_DECIMALS)).toEqual('12.34 K');
		// Millions:
		expect(formatBalance(toNanos('1234000'), IOTA_DECIMALS)).toEqual('1.23 M');
		// Billions:
		expect(formatBalance(toNanos('1234000000'), IOTA_DECIMALS)).toEqual('1.23 B');
	});

	it('formats integer amounts with full CoinFormat', () => {
		expect(formatBalance(toNanos('1'), IOTA_DECIMALS, CoinFormat.FULL)).toEqual('1');
		expect(formatBalance(toNanos('1.123456789'), IOTA_DECIMALS, CoinFormat.FULL)).toEqual(
			'1.123456789',
		);
		expect(formatBalance(toNanos('9999.9999'), IOTA_DECIMALS, CoinFormat.FULL)).toEqual('9,999.9999');
		expect(formatBalance(toNanos('10000'), IOTA_DECIMALS, CoinFormat.FULL)).toEqual('10,000');
		expect(formatBalance(toNanos('12345'), IOTA_DECIMALS, CoinFormat.FULL)).toEqual('12,345');
		expect(formatBalance(toNanos('1234000'), IOTA_DECIMALS, CoinFormat.FULL)).toEqual('1,234,000');
		expect(formatBalance(toNanos('1234000000'), IOTA_DECIMALS, CoinFormat.FULL)).toEqual(
			'1,234,000,000',
		);
	});
});
