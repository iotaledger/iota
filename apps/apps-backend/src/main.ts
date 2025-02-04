// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AppFactory } from './AppFactory.js';

async function bootstrap() {
  const { appPromise } = AppFactory.create();
  const app = await appPromise;

  await app.listen(process.env.PORT ?? 3003);
}
bootstrap();