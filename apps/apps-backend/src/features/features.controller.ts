// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get, Query } from '@nestjs/common';
import { FeaturesService } from './features.service';

@Controller('/api/features')
export class FeaturesController {
    constructor(private readonly featuresService: FeaturesService) {}

    @Get('/staging')
    getStagingFeatures(@Query('version') version?: string) {
        return this.featuresService.getStagingFeatures(version);
    }

    @Get('/production')
    getProductionFeatures(@Query('version') version?: string) {
        return this.featuresService.getProductionFeatures(version);
    }

    @Get('/apps')
    getAppsFeatures() {
        return {
            status: 200,
            apps: [], // Note: we'll add wallet dapps when evm will be ready
            dateUpdated: new Date().toISOString(),
        };
    }
}
