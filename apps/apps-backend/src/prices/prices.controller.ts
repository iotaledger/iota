// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get, Param } from '@nestjs/common';

import { APPS_BACKEND_SUI_URL } from '../constants';

@Controller()
export class PricesController {
	@Get('cetus/:coin')
	async getTokenPrice(@Param('coin') coin: string) {
		const resp = await fetch(`${APPS_BACKEND_SUI_URL}/cetus/${coin}`);
		if (!resp.ok) {
			throw new Error('Failed to fetch the data');
		}
		const text = await resp.text();
		return text;
	}
}
