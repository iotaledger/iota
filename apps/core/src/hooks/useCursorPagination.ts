// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { type InfiniteData, type UseInfiniteQueryResult } from '@tanstack/react-query';

interface PaginationProps {
    hasFirst: boolean;
    hasPrev: boolean;
    hasNext: boolean;
    onFirst(): void;
    onPrev(): void;
    onNext(): void;
}

interface CursorPaginationProps extends PaginationProps {
    currentPage: number;
}

export interface PaginationResponse<Cursor = string> {
    nextCursor: Cursor | null;
    hasNextPage: boolean;
}

export function useCursorPagination<T>(query: UseInfiniteQueryResult<InfiniteData<T>>) {
    const [currentPage, setCurrentPage] = useState(0);

    return {
        ...query,
        data: query.data?.pages[currentPage],
        pagination: {
            onFirst: () => setCurrentPage(0),
            onNext: () => {
                if (!query.data || query.isFetchingNextPage) {
                    return;
                }

                // Make sure we are at the end before fetching another page
                if (currentPage >= query.data.pages.length - 1) {
                    query.fetchNextPage();
                }

                setCurrentPage(currentPage + 1);
            },
            onPrev: () => {
                setCurrentPage(Math.max(currentPage - 1, 0));
            },
            hasFirst: currentPage !== 0,
            hasNext:
                !query.isFetchingNextPage &&
                (currentPage < (query.data?.pages.length ?? 0) - 1 || !!query.hasNextPage),
            hasPrev: currentPage !== 0,
            currentPage,
        } satisfies CursorPaginationProps,
    };
}
