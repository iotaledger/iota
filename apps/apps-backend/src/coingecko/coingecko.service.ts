// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Inject, Injectable } from '@nestjs/common';
import { Cron, CronExpression } from '@nestjs/schedule';
import axios from 'axios';
import { CACHE_MANAGER, Cache } from '@nestjs/cache-manager';

import { tokenPriceKey } from '../cache/cache.constants';

@Injectable()
export class CoinGeckoService {
    private readonly baseUrl = 'https://api.coingecko.com';

    constructor(@Inject(CACHE_MANAGER) private cacheManager: Cache) {}

    async getTokenPrice(coinId: string, currency: string = 'usd'): Promise<number> {
        try {
            const response = await axios.get(`${this.baseUrl}/api/v3/simple/price`, {
                params: {
                    ids: coinId,
                    vs_currencies: currency,
                },
            });
            return response.data[coinId][currency];
        } catch (error) {
            throw new Error('Failed to fetch token prices from CoinGecko');
        }
    }

    @Cron(CronExpression.EVERY_HOUR)
    async refreshCachedPrices() {
        const coins = ['iota'];
        const currency = 'usd';
        for (const coin of coins) {
            const price = await this.getTokenPrice(coin, currency);
            await this.cacheManager.set(tokenPriceKey(coin), price);
        }
    }
}
