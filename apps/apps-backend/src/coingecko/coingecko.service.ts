// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Inject, Injectable } from '@nestjs/common';
import { Cron, CronExpression } from '@nestjs/schedule';
import { CACHE_MANAGER, Cache } from '@nestjs/cache-manager';

import { TOKEN_PRICE_COINS, TOKEN_PRICE_CURRENCY, tokenPriceKey } from '../constants';

@Injectable()
export class CoinGeckoService {
    private readonly baseUrl = 'https://api.coingecko.com';

    constructor(@Inject(CACHE_MANAGER) private cacheManager: Cache) {}

    async getTokenPrice(coinId: string, currency: string = 'usd'): Promise<number> {
        const url = new URL(`${this.baseUrl}/api/v3/simple/price`);
        url.searchParams.append('ids', coinId);
        url.searchParams.append('vs_currencies', currency);

        const response = await fetch(url.toString());
        if (!response.ok) {
            throw new Error('Failed to fetch token prices from CoinGecko');
        }
        const data = await response.json();
        return data[coinId][currency];
    }

    @Cron(CronExpression.EVERY_HOUR)
    async refreshCachedPrices() {
        for (const coin of TOKEN_PRICE_COINS) {
            const price = await this.getTokenPrice(coin, TOKEN_PRICE_CURRENCY);
            await this.cacheManager.set(tokenPriceKey(coin), price);
        }
    }
}
