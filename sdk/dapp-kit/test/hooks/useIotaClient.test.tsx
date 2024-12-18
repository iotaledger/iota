// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { renderHook } from '@testing-library/react';

import { useIotaClient } from '../../src/index.js';
import { createIotaClientContextWrapper } from '../test-utils.js';

describe('useIotaClient', () => {
    test('throws without a IotaClientContext', () => {
        expect(() => renderHook(() => useIotaClient())).toThrowError(
            'Could not find IotaClientContext. Ensure that you have set up the IotaClientProvider',
        );
    });

    test('returns a IotaClient', () => {
        const iotaClient = new IotaClient({ url: getFullnodeUrl('localnet') });
        const wrapper = createIotaClientContextWrapper(iotaClient);
        const { result } = renderHook(() => useIotaClient(), { wrapper });

        expect(result.current).toBe(iotaClient);
    });
});
