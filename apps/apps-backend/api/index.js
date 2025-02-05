// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NestFactory, HttpAdapterHost } from '@nestjs/core';
import { AppModule } from '../dist/apps-backend/src/app.module';

let app;

/**
 * Vercel serverless entry point for the NestJS application.
 *
 * This file must be in the /api folder as per Vercel's conventions for serverless functions.
 * The /api folder is a special directory that Vercel uses to automatically create serverless
 * function endpoints. Each file in this directory becomes a route handler.
 *
 * In this case, we're using it as a bridge between Vercel's serverless environment
 * and our NestJS application. It:
 * 1. Creates the NestJS application instance (once, on first request)
 * 2. Sets up CORS
 * 3. Routes all incoming HTTP requests to the NestJS application
 *
 * The AppModule is imported from the dist folder since this code runs after the NestJS
 * application has been built.
 */
export default async function handler(req, res) {
    if (!app) {
        app = await NestFactory.create(AppModule);

        app.enableCors({
            origin: '*',
            methods: 'GET,HEAD,PUT,PATCH,POST,DELETE',
            credentials: true,
        });

        await app.init();
    }

    const adapterHost = app.get(HttpAdapterHost);
    const httpAdapter = adapterHost.httpAdapter;
    const instance = httpAdapter.getInstance();

    instance(req, res);
}
