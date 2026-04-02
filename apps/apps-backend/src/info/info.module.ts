// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Module } from '@nestjs/common';
import { InfoController } from './info.controller';

@Module({
    controllers: [InfoController],
})
export class InfoModule {}
