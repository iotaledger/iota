// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { HttpAdapterHost, NestFactory } from '@nestjs/core';
import { AppModule } from './app.module';
import { INestApplication } from '@nestjs/common';

let server: INestApplication;

export default async function handler(req: Request, res: Response) {
    if (!server) {
        server = await NestFactory.create(AppModule);

        server.enableCors({
            origin: '*',
            methods: 'GET,HEAD,PUT,PATCH,POST,DELETE',
            credentials: true,
        });

        await server.init();
    }

    const adapterHost = server.get(HttpAdapterHost);
    const httpAdapter = adapterHost.httpAdapter;
    const instance = httpAdapter.getInstance();

    instance(req, res);
}
