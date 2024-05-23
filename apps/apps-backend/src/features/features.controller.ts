// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get } from '@nestjs/common';

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
}
