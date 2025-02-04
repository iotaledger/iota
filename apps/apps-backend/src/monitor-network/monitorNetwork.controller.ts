// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get, Query } from '@nestjs/common';

type Project = 'LALA' | 'EXPLORER';

@Controller('/monitor-network')
export class MonitorNetworkController {
    @Get('/')
    async getMonitorNetwork(@Query('project') project: Project) {
        if (project === 'LALA') {
            return {
                degraded: false,
            };
        }
        return {
            degraded: false,
        };
    }
}
