// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Module } from '@nestjs/common';
import { HealthController } from './health.controller';

@Module({
    controllers: [HealthController],
})
export class HealthModule {}
