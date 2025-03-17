// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IsEnum, IsOptional } from 'class-validator';

export class RestrictedRequestDto {
    @IsOptional()
    @IsEnum(['staging', 'production'], {
        message: 'env must be either "staging" or "production"',
    })
    env?: 'staging' | 'production';
}
