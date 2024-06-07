// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Module } from '@nestjs/common';

import { MonitorNetworkController } from './monitor-network.controller';

@Module({
    controllers: [MonitorNetworkController],
})
export class MonitorNetworkModule {}
