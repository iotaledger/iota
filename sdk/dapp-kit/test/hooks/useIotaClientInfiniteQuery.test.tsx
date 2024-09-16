// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { act, renderHook, waitFor } from '@testing-library/react';

import { useIotaClientInfiniteQuery } from '../../src/hooks/useIotaClientInfiniteQuery.js';
import { createWalletProviderContextWrapper } from '../test-utils.js';

describe('useIotaClientInfiniteQuery', () => {
    it('should fetch data', async () => {
        const iotaClient = new IotaClient({ url: getFullnodeUrl('mainnet') });
        const wrapper = createWalletProviderContextWrapper({}, iotaClient);

        const queryTransactionBlocks = vi.spyOn(iotaClient, 'queryTransactionBlocks');

        const pages = [
            {
                data: [{ digest: '0x123' }],
                hasNextPage: true,
                nextCursor: 'page2',
            },
            {
                data: [{ digest: '0x456' }],
                hasNextPage: false,
                nextCursor: null,
            },
        ];

        queryTransactionBlocks.mockResolvedValueOnce(pages[0]);

        const { result } = renderHook(
            () =>
                useIotaClientInfiniteQuery('queryTransactionBlocks', {
                    filter: {
                        FromAddress: '0x123',
                    },
                }),
            { wrapper },
        );

        expect(result.current.isPending).toBe(true);
        expect(result.current.isError).toBe(false);
        expect(result.current.data).toBe(undefined);
        expect(queryTransactionBlocks).toHaveBeenCalledWith({
            cursor: null,
            filter: {
                FromAddress: '0x123',
            },
        });

        await waitFor(() => expect(result.current.isSuccess).toBe(true));

        expect(result.current.isPending).toBe(false);
        expect(result.current.isError).toBe(false);
        expect(result.current.data).toEqual({
            pageParams: [null],
            pages: [pages[0]],
        });

        queryTransactionBlocks.mockResolvedValueOnce(pages[1]);

        await act(() => {
            result.current.fetchNextPage();
        });

        await waitFor(() => expect(result.current.isFetchingNextPage).toBe(false));

        expect(result.current.isPending).toBe(false);
        expect(result.current.isError).toBe(false);
        expect(result.current.data).toEqual({
            pageParams: [null, 'page2'],
            pages: [pages[0], pages[1]],
        });
        expect(result.current.data?.pages[0].data[0].digest).toBe('0x123');

        expect(queryTransactionBlocks).toHaveBeenCalledWith({
            filter: {
                FromAddress: '0x123',
            },
            cursor: 'page2',
        });
    });
});
