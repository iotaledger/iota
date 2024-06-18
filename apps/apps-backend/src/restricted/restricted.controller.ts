// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Header, HttpStatus, Post, Res } from '@nestjs/common';
import { Response } from 'express';

@Controller('/api/restricted')
export class RestrictedController {
    @Post('/')
    @Header('Cache-Control', 'max-age=0, must-revalidate')
    checkRestrictions(@Res() res: Response) {
        // here we can check for location restrictions and respond accordingly
        res.status(HttpStatus.OK).send();
    }
}
