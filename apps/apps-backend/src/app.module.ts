// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Module } from '@nestjs/common';
import { ConfigModule } from '@nestjs/config';

import { AnalyticsModule } from './analytics/analytics.module';
import { FeaturesModule } from './features/features.module';
import { MonitorNetworkModule } from './monitor-network/monitor-network.module';
import { PricesModule } from './prices/prices.module';

@Module({
    imports: [
        PricesModule,
        FeaturesModule,
        MonitorNetworkModule,
        AnalyticsModule,
        ConfigModule.forRoot({
            isGlobal: true,
            envFilePath: '.env',
            expandVariables: true,
        }),
    ],
})
export class AppModule {}
