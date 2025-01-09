// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Module } from '@nestjs/common';

import { MonitorNetworkController } from './monitorNetwork.controller';

@Module({
    controllers: [MonitorNetworkController],
})
export class MonitorNetworkModule {}
