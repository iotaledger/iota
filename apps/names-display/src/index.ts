// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import './env.js';

import { readFileSync } from 'node:fs';
import cors from '@fastify/cors';
import Fastify, { FastifyReply, FastifyRequest } from 'fastify';
import * as z from 'zod';

import { isInvalidated } from './utils/invalidation.js';
import { renderSvg } from './utils/renderSvg.js';

const baseSvgTemplate = readFileSync(new URL('./svg/base.svg', import.meta.url), 'utf-8');
const expiredSvgTemplate = readFileSync(new URL('./svg/expired.svg', import.meta.url), 'utf-8');
const invalidatedSvgTemplate = readFileSync(
    new URL('./svg/invalidated.svg', import.meta.url),
    'utf-8',
);

export const Params = z.object({
    name: z.string(),
    timestamp: z.coerce.number(),
    objectId: z.string().optional(),
});

const fastify = Fastify({ logger: true, maxParamLength: 1000 });

await fastify.register(cors, {
    origin: '*',
    methods: ['GET', 'HEAD'],
});

function isExpired(timestamp: number): boolean {
    return timestamp < Date.now();
}

function getTemplate(timestamp: number, invalidated: boolean): string {
    if (invalidated) {
        return invalidatedSvgTemplate;
    }
    if (isExpired(timestamp)) return expiredSvgTemplate;
    return baseSvgTemplate;
}

const handleRequest = async (req: FastifyRequest, reply: FastifyReply) => {
    const params = await z.parseAsync(Params, req.params);
    const invalidated = isInvalidated(params.objectId);
    const templateSvg = getTemplate(params.timestamp, invalidated);
    try {
        const svg = renderSvg({ ...params, template: templateSvg, invalidated });
        reply.type('image/svg+xml').code(200);
        return svg;
    } catch (e) {
        console.error(e);
        reply.code(500);
        return 'Error generating SVG';
    }
};

fastify.get('/:name/:timestamp', handleRequest);
fastify.get('/:name/:timestamp/:objectId', handleRequest);

fastify.listen({ host: '0.0.0.0', port: Number(process.env.PORT) || 3000 });
