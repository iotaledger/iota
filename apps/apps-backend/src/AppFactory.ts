// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NestFactory } from "@nestjs/core";
import { ExpressAdapter } from "@nestjs/platform-express";
import express, { Request, Response, Express } from "express";
import { AppModule } from "./app.module";
import { INestApplication } from "@nestjs/common";

export class AppFactory {
  static create(): {
    appPromise: Promise<INestApplication<any>>;
    expressApp: Express;
  } {
    const expressApp = express();
    const adapter = new ExpressAdapter(expressApp);
    const appPromise = NestFactory.create(AppModule, adapter);

    appPromise
      .then((app) => {
        app.enableCors({
          origin: "*",
          methods: "GET,HEAD,PUT,PATCH,POST,DELETE",
          credentials: true,
        });

        app.init();
      })
      .catch((err) => {
        throw err;
      });

    // IMPORTANT This express application-level middleware makes sure the NestJS app is fully initialized
    expressApp.use((req: Request, res: Response, next) => {
      appPromise
        .then(async (app) => {
          await app.init();
          next();
        })
        .catch((err) => next(err));
    });

    return { appPromise, expressApp };
  }
}