// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get, Query } from '@nestjs/common';

import { developmentFeatures } from './features.mock';

@Controller('/api/features')
export class FeaturesController {
    @Get('/development')
    getDevelopmentFeatures() {
        return {
            status: 200,
            features: developmentFeatures,
            dateUpdated: new Date().toISOString(),
        };
    }

    @Get('/production')
    getProductionFeatures() {
        return {
            status: 200,
            features: developmentFeatures,
            dateUpdated: new Date().toISOString(),
        };
    }

    @Get('/apps')
    getAppsFeatures(@Query('network') network: string) {
        const apps = developmentFeatures['wallet-dapps'].rules
            .filter((rule) => rule.condition.network === network)
            .reduce((acc, rule) => [...acc, ...rule.force], []);

        return {
            status: 200,
            apps,
            dateUpdated: new Date().toISOString(),
        };
    }
}
