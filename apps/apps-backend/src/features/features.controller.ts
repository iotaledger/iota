// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get } from '@nestjs/common';
import { Feature } from '@iota/core/constants/features.enum';

@Controller('/api/features')
export class FeaturesController {
    @Get('/development')
    getDevelopmentFeatures() {
        console.log(Feature);
        return {
            status: 200,
            features: {},
            dateUpdated: new Date().toISOString(),
        };
    }

    @Get('/production')
    getProductionFeatures() {
        return {
            status: 200,
            features: {},
            dateUpdated: new Date().toISOString(),
        };
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
