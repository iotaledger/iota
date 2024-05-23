// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';

export const seedValidation = z
	.string()
	.trim()
	.nonempty('Seed is required.')
	.transform((seed, context) => {
		return seed;
	});
