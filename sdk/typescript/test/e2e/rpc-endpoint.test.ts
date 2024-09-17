// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { beforeAll, describe, expect, it } from 'vitest';

import { IotaGasData } from '../../src/client';
import { setup, TestToolbox } from './utils/setup';

describe('Invoke any RPC endpoint', () => {
    let toolbox: TestToolbox;

    beforeAll(async () => {
        toolbox = await setup();
    });

    it('iotax_getOwnedObjects', async () => {
        const gasObjectsExpected = await toolbox.client.getOwnedObjects({
            owner: toolbox.address(),
        });
        const gasObjects = await toolbox.client.call<{ data: IotaGasData }>(
            'iotax_getOwnedObjects',
            [toolbox.address()],
        );
        expect(gasObjects.data).toStrictEqual(gasObjectsExpected.data);
    });

    it('iota_getObjectOwnedByAddress Error', async () => {
        expect(toolbox.client.call('iotax_getOwnedObjects', [])).rejects.toThrowError();
    });

    it('iotax_getCommitteeInfo', async () => {
        const committeeInfoExpected = await toolbox.client.getCommitteeInfo();

        const committeeInfo = await toolbox.client.call('iotax_getCommitteeInfo', []);

        expect(committeeInfo).toStrictEqual(committeeInfoExpected);
    });
});
