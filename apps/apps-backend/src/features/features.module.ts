// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Module } from '@nestjs/common';

import { FeaturesController } from './features.controller';

@Module({
    controllers: [FeaturesController],
})
export class FeaturesModule {}
