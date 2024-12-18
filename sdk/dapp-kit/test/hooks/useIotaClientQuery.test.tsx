// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { renderHook, waitFor } from '@testing-library/react';

import { useIotaClientQuery } from '../../src/hooks/useIotaClientQuery.js';
import { createWalletProviderContextWrapper } from '../test-utils.js';

describe('useIotaClientQuery', () => {
    it('should fetch data', async () => {
        const iotaClient = new IotaClient({ url: getFullnodeUrl('mainnet') });
        const wrapper = createWalletProviderContextWrapper({}, iotaClient);

        const queryTransactionBlocks = vi.spyOn(iotaClient, 'queryTransactionBlocks');

        queryTransactionBlocks.mockResolvedValueOnce({
            data: [{ digest: '0x123' }],
            hasNextPage: true,
            nextCursor: 'page2',
        });

        const { result } = renderHook(
            () =>
                useIotaClientQuery('queryTransactionBlocks', {
                    filter: {
                        FromAddress: '0x123',
                    },
                }),
            { wrapper },
        );

        expect(result.current.isLoading).toBe(true);
        expect(result.current.isError).toBe(false);
        expect(result.current.data).toBe(undefined);
        expect(queryTransactionBlocks).toHaveBeenCalledWith({
            filter: {
                FromAddress: '0x123',
            },
        });

        await waitFor(() => expect(result.current.isSuccess).toBe(true));

        expect(result.current.isLoading).toBe(false);
        expect(result.current.isError).toBe(false);
        expect(result.current.data).toEqual({
            data: [{ digest: '0x123' }],
            hasNextPage: true,
            nextCursor: 'page2',
        });
    });
});
