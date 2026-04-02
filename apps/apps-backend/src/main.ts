// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ConfigService } from '@nestjs/config';
import { NestFactory } from '@nestjs/core';

import { AppModule } from './app.module';

async function bootstrap() {
    const app = await NestFactory.create(AppModule);
    const configService = app.get(ConfigService);

    function getRequiredEnv(key: string): string {
        const value = configService.get<string>(key);
        if (!value) {
            throw new Error(`${key} is not configured`);
        }
        return value;
    }

    getRequiredEnv('DEPLOY_TYPE');
    getRequiredEnv('STAGING_APPS_BACKEND');
    getRequiredEnv('PROD_APPS_BACKEND');

    app.enableCors({
        origin: '*',
        methods: 'GET,HEAD,PUT,PATCH,POST,DELETE',
        credentials: true,
    });

    await app.listen(configService.get('PORT') || 3003);
}
bootstrap();
