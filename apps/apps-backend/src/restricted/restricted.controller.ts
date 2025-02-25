// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Header, HttpStatus, Post, Res, Body } from '@nestjs/common';
import { Response } from 'express';
import { RestrictedRequestDto } from './restricted.dto';

@Controller('/api/restricted')
export class RestrictedController {
    @Post('/')
    @Header('Cache-Control', 'max-age=0, must-revalidate')
    checkRestrictions(@Body() body: RestrictedRequestDto, @Res() res: Response) {
        const restrictedFlags = {
            staging: false,
            production: false,
        };

        const isRestricted = body.env ? restrictedFlags[body.env] : false;

        if (isRestricted) {
            res.status(HttpStatus.FORBIDDEN).send();
        } else {
            res.status(HttpStatus.OK).send();
        }
    }
}
