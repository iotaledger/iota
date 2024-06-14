// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Module } from '@nestjs/common';
import { CoinGeckoService } from './coingecko.service';

@Module({
    imports: [],
    providers: [CoinGeckoService],
    exports: [CoinGeckoService],
})
export class CoinGeckoModule {}
