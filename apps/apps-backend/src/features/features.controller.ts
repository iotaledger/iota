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
            .reduce(
                (acc, rule) => {
                    const { force } = rule;
                    force.forEach((item) => {
                        if (!acc.map[item.name]) {
                            acc.result.push(item);
                            acc.map[item.name] = true;
                        }
                    });

                    return acc;
                },
                { result: [], map: {} },
            );

        return {
            status: 200,
            apps: apps.result,
            dateUpdated: new Date().toISOString(),
        };
    }
}
