// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonSize, ButtonType } from '@iota/apps-ui-kit';
import { ArrowLeft, ArrowRight, DoubleArrowLeft } from '@iota/apps-ui-icons';
import { useState } from 'react';

interface PaginationProps {
    hasFirst: boolean;
    hasPrev: boolean;
    hasNext: boolean;
    onFirst(): void;
    onPrev(): void;
    onNext(): void;
}

export interface PaginationResponse<Cursor = string> {
    nextCursor: Cursor | null;
    hasNextPage: boolean;
}

/** @deprecated Prefer `useCursorPagination` from core + `useInfiniteQuery` for pagination. */
export function usePaginationStack<Cursor = string>() {
    const [stack, setStack] = useState<Cursor[]>([]);

    return {
        cursor: stack.at(-1),
        props({
            hasNextPage = false,
            nextCursor = null,
        }: Partial<PaginationResponse<Cursor>> = {}): PaginationProps {
            return {
                hasFirst: stack.length > 0,
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

export function Pagination({
    hasNext,
    hasPrev,
    onFirst,
    onPrev,
    onNext,
}: PaginationProps): JSX.Element {
    return (
        <div className="flex gap-2">
            <Button
                type={ButtonType.Secondary}
                size={ButtonSize.Small}
                icon={<DoubleArrowLeft />}
                onClick={onFirst}
                disabled={!hasPrev}
            />
            <Button
                type={ButtonType.Secondary}
                size={ButtonSize.Small}
                icon={<ArrowLeft />}
                onClick={onPrev}
                disabled={!hasPrev}
            />
            <Button
                type={ButtonType.Secondary}
                size={ButtonSize.Small}
                icon={<ArrowRight />}
                disabled={!hasNext}
                onClick={onNext}
            />
        </div>
    );
}
