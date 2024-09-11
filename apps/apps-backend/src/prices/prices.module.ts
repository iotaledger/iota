// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Module } from '@nestjs/common';

import { PricesController } from './prices.controller';
import { CoinGeckoModule } from '../coingecko/coingecko.module';

@Module({
    imports: [CoinGeckoModule],
    controllers: [PricesController],
})
export class PricesModule {}
