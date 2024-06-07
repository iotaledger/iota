// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Module } from '@nestjs/common';

import { AnalyticsController } from './analytics.controller';

@Module({
    controllers: [AnalyticsController],
})
export class AnalyticsModule {}
