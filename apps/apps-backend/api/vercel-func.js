// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NestFactory, HttpAdapterHost } from '@nestjs/core';
import { AppModule } from '../dist/apps-backend/src/app.module';

let app;

export default async function handler(req, res) {
    if (!app) {
        app = await NestFactory.create(AppModule);

        app.enableCors({
            origin: process.env.CORS_ORIGINS || true,
            credentials: true,
        });

        await app.init();
    }

    const adapterHost = app.get(HttpAdapterHost);
    const httpAdapter = adapterHost.httpAdapter;
    const instance = httpAdapter.getInstance();

    instance(req, res);
}
