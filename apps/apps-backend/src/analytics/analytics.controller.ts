// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get } from '@nestjs/common';

@Controller()
export class AnalyticsController {
    @Get('product-analytics')
    getProductAnalytics() {
        return;
    }
}
