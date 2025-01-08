// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaEvent } from '@iota/iota-sdk/client';
import { ParsedJson } from '../../interfaces';

export function parseEventJson<T extends ParsedJson>(event: IotaEvent): T {
    return event.parsedJson as T;
}
