// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Module } from '@nestjs/common';
import { CacheModule } from '@nestjs/cache-manager';
import { ScheduleModule } from '@nestjs/schedule';
import { ConfigModule } from '@nestjs/config';

import { AnalyticsModule } from './analytics/analytics.module';
import { FeaturesModule } from './features/features.module';
import { MonitorNetworkModule } from './monitor-network/monitorNetwork.module';
import { PricesModule } from './prices/prices.module';
import { RestrictedModule } from './restricted/restricted.module';
import { HealthModule } from './health/health.module';

@Module({
    imports: [
        PricesModule,
        FeaturesModule,
        MonitorNetworkModule,
        AnalyticsModule,
        RestrictedModule,
        ScheduleModule.forRoot(),
        ConfigModule.forRoot({
            isGlobal: true,
            envFilePath: '.env',
            expandVariables: true,
        }),
        CacheModule.register({
            isGlobal: true,
            ttl: 3600,
            max: 100,
        }),
        HealthModule,
    ],
})
export class AppModule {}
