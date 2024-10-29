// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Test, TestingModule } from '@nestjs/testing';
import { INestApplication } from '@nestjs/common';
import * as request from 'supertest';
import { AppModule } from '../src/app.module';

describe('Health Check (e2e)', () => {
    let app: INestApplication;

    beforeAll(async () => {
        const moduleFixture: TestingModule = await Test.createTestingModule({
            imports: [AppModule],
        }).compile();

        app = moduleFixture.createNestApplication();
        await app.init();
    });

    afterAll(async () => {
        await app.close();
    });

    it('/health (GET)', async () => {
        const DEPLOYED_URL = process.env.DEPLOYED_URL || null;

        if (DEPLOYED_URL) {
            await request(DEPLOYED_URL).get('/health').expect(200).expect({ status: 'ok' });
        } else {
            await request(app.getHttpServer()).get('/health').expect(200).expect({ status: 'ok' });
        }
    });
});
