// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung 
// SPDX-License-Identifier: Apache-2.0

import { PrismaClient } from '@prisma/client';

const globalForPrisma = globalThis as unknown as { prisma: PrismaClient };

export const prisma =
	globalForPrisma.prisma ||
	new PrismaClient({
		log: ['query'],
	});

if (process.env.NODE_ENV !== 'production') globalForPrisma.prisma = prisma;
