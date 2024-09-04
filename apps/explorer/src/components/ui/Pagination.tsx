// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonSize, ButtonType } from '@iota/apps-ui-kit';
import { DoubleArrowLeft, ArrowLeft, ArrowRight } from '@iota/ui-icons';
import { type InfiniteData, type UseInfiniteQueryResult } from '@tanstack/react-query';
import { useState } from 'react';

interface PaginationProps {
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
            hasNext:
                !query.isFetchingNextPage &&
                (currentPage < (query.data?.pages.length ?? 0) - 1 || !!query.hasNextPage),
            hasPrev: currentPage !== 0,
            currentPage,
        } satisfies CursorPaginationProps,
    };
}

/** @deprecated Prefer `useCursorPagination` + `useInfiniteQuery` for pagination. */
export function usePaginationStack<Cursor = string>() {
    const [stack, setStack] = useState<Cursor[]>([]);

    return {
        cursor: stack.at(-1),
        props({
            hasNextPage = false,
            nextCursor = null,
        }: Partial<PaginationResponse<Cursor>> = {}): PaginationProps {
            return {
                hasPrev: stack.length > 0,
                hasNext: hasNextPage,
                onFirst() {
                    setStack([]);
                },
                onNext() {
                    if (nextCursor && hasNextPage) {
                        setStack((stack) => [...stack, nextCursor]);
                    }
                },
                onPrev() {
                    setStack((stack) => stack.slice(0, -1));
                },
            };
        },
    };
}

interface PaginationButtonProps {
    label: string;
    icon: typeof DoubleArrowLeft;
    disabled: boolean;
    onClick(): void;
}

function PaginationButton({
    label,
    icon: Icon,
    disabled,
    onClick,
}: PaginationButtonProps): JSX.Element {
    return (
        <Button
            size={ButtonSize.Small}
            type={ButtonType.Secondary}
            aria-label={label}
            disabled={disabled}
            onClick={onClick}
            icon={<Icon />}
        />
    );
}

export function Pagination({
    hasNext,
    hasPrev,
    onFirst,
    onPrev,
    onNext,
}: PaginationProps): JSX.Element {
    return (
        <div className="flex gap-2">
            <PaginationButton
                label="Go to First"
                icon={DoubleArrowLeft}
                disabled={!hasPrev}
                onClick={onFirst}
            />
            <PaginationButton
                label="Previous"
                icon={ArrowLeft}
                disabled={!hasPrev}
                onClick={onPrev}
            />
            <PaginationButton label="Next" icon={ArrowRight} disabled={!hasNext} onClick={onNext} />
        </div>
    );
}
