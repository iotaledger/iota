// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Header, HttpStatus, Post, Res } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { Response } from 'express';

@Controller('/api/restricted')
export class RestrictedController {
    constructor(private readonly configService: ConfigService) {}

    @Post('/')
    @Header('Cache-Control', 'max-age=0, must-revalidate')
    checkRestrictions(@Res() res: Response) {
        const deployType = this.configService.get<string>('DEPLOY_TYPE');

        const restrictedFlags: Record<string, boolean> = {
            staging: false,
            rc: false,
            production: false,
        };

        const isRestricted = restrictedFlags[deployType] ?? false;

        if (isRestricted) {
            res.status(HttpStatus.FORBIDDEN).send();
        } else {
            res.status(HttpStatus.OK).send();
        }
    }
}
