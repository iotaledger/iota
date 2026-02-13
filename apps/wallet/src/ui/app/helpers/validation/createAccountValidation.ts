// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';
import { AccountsFormType } from '../../components/accounts/AccountsFormContext';

export const createAccountValidation = z
    .object({
        type: z.nativeEnum(AccountsFormType),
        password: z.string().optional(),
    })
    .superRefine((data, ctx) => {
        // Password is required for all types except MnemonicSource and SeedSource
        const requiresPassword =
            data.type !== AccountsFormType.MnemonicSource &&
            data.type !== AccountsFormType.SeedSource;

        if (requiresPassword && !data.password) {
            ctx.addIssue({
                code: z.ZodIssueCode.custom,
                message: 'Password is required',
                path: ['password'],
            });
        }
    });
