// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';

@Controller('/api/info')
export class InfoController {
    constructor(private readonly configService: ConfigService) {}

    @Get()
    getInfo() {
        const deployType = this.configService.get<string>('DEPLOY_TYPE');

        if (!deployType) {
            throw new Error('DEPLOY_TYPE is not configured');
        }

        return { deployType };
    }
}
