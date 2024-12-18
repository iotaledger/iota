// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { openTransportReplayer, RecordStore } from '@ledgerhq/hw-transport-mocker';
import { expect, test } from 'vitest';

import Iota from '../src/Iota';

test('Iota init', async () => {
    const transport = await openTransportReplayer(RecordStore.fromString(''));
    const pkt = new Iota(transport);
    expect(pkt).not.toBe(undefined);
});
