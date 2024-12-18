// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Module } from '@nestjs/common';
import { RestrictedController } from './restricted.controller';

@Module({
    controllers: [RestrictedController],
})
export class RestrictedModule {}
