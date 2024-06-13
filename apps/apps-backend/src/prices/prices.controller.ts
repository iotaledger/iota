// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get, Inject, Param } from '@nestjs/common';
import { Cache, CACHE_MANAGER } from '@nestjs/cache-manager';
import { CoinGeckoService } from '../coingecko/coingecko.service';
import { tokenPriceKey } from '../cache/cache.constants';

const ONE_HOUR_IN_MS = 1000 * 60 * 60;

@Controller()
export class PricesController {
    constructor(
        @Inject(CACHE_MANAGER) private cacheManager: Cache,
        private coinGeckoService: CoinGeckoService,
    ) {}

    @Get('cetus/:coin')
    async getTokenPrice(@Param('coin') coin: string) {
        coin = coin.toLowerCase();
        const allowedCoins = ['iota'];

        if (!allowedCoins.includes(coin)) {
            throw new Error('Invalid coin');
        }

        const cacheKey = tokenPriceKey(coin);
        const tokenPriceCached = await this.cacheManager.get<number>(cacheKey);

        if (!tokenPriceCached) {
            const tokenPriceCg = await this.coinGeckoService.getTokenPrice(coin);
            await this.cacheManager.set(cacheKey, tokenPriceCg, ONE_HOUR_IN_MS);
            return {
                price: tokenPriceCg,
            };
        }

        return {
            price: tokenPriceCached,
        };
    }
}
