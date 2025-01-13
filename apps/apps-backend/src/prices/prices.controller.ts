// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get, Inject, Param, Query } from '@nestjs/common';
import { Cache, CACHE_MANAGER } from '@nestjs/cache-manager';
import { CoinGeckoService } from '../coingecko/coingecko.service';
import { TOKEN_PRICE_COINS, tokenPriceKey } from '../constants';
import { FiatTokenName } from '@iota/core/enums/fiatTokenName.enums';
import { Network } from '@iota/iota-sdk/client';

const ONE_HOUR_IN_MS = 1000 * 60 * 60;

@Controller()
export class PricesController {
    constructor(
        @Inject(CACHE_MANAGER) private cacheManager: Cache,
        private coinGeckoService: CoinGeckoService,
    ) {}

    @Get('coin-price/:coin')
    async getTokenPrice(@Param('coin') coin: FiatTokenName, @Query('network') network: Network) {
        if (!TOKEN_PRICE_COINS.includes(coin)) {
            throw new Error('Invalid coin');
        }
        if (network !== Network.Mainnet) {
            return {
                price: null,
            };
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
