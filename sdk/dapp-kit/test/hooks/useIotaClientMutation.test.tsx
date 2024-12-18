// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { act, renderHook, waitFor } from '@testing-library/react';

import { useIotaClientMutation } from '../../src/hooks/useIotaClientMutation.js';
import { createWalletProviderContextWrapper } from '../test-utils.js';

describe('useIotaClientMutation', () => {
    it('should fetch data', async () => {
        const iotaClient = new IotaClient({ url: getFullnodeUrl('mainnet') });
        const wrapper = createWalletProviderContextWrapper({}, iotaClient);

        const queryTransactionBlocks = vi.spyOn(iotaClient, 'queryTransactionBlocks');

        queryTransactionBlocks.mockResolvedValueOnce({
            data: [{ digest: '0x123' }],
            hasNextPage: true,
            nextCursor: 'page2',
        });

        const { result } = renderHook(() => useIotaClientMutation('queryTransactionBlocks'), {
            wrapper,
        });

        act(() => {
            result.current.mutate({
                filter: {
                    FromAddress: '0x123',
                },
            });
        });

        await waitFor(() => expect(result.current.status).toBe('success'));

        expect(queryTransactionBlocks).toHaveBeenCalledWith({
            filter: {
                FromAddress: '0x123',
            },
        });
        expect(result.current.isPending).toBe(false);
        expect(result.current.isError).toBe(false);
        expect(result.current.data).toEqual({
            data: [{ digest: '0x123' }],
            hasNextPage: true,
            nextCursor: 'page2',
        });
    });
});
