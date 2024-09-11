// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type TransactionFilter } from '@iota/iota-sdk/client';
import { Heading, RadioGroup, RadioGroupItem } from '@iota/ui';
import { type Dispatch, type SetStateAction, useReducer, useState } from 'react';

import clsx from 'clsx';
import { Pagination, PlaceholderTable, TableCard } from '~/components/ui';
import {
    DEFAULT_TRANSACTIONS_LIMIT,
    useGetTransactionBlocks,
} from '~/hooks/useGetTransactionBlocks';
import { ObjectFilterValue } from '~/lib/enums';
import { genTableDataFromTxData } from '../transactions/TxCardUtils';

type TransactionBlocksForAddressProps = {
    address: string;
    filter?: ObjectFilterValue;
    header?: string;
};

enum PageAction {
    Next,
    Prev,
    First,
}

type TransactionBlocksForAddressActionType = {
    type: PageAction;
    filterValue: ObjectFilterValue;
};

type PageStateByFilterMap = {
    [ObjectFilterValue.Input]: number;
    [ObjectFilterValue.Changed]: number;
};

const FILTER_OPTIONS: { label: string; value: ObjectFilterValue }[] = [
    { label: 'Input Objects', value: ObjectFilterValue.Input },
    { label: 'Updated Objects', value: ObjectFilterValue.Changed },
];

function reducer(
    state: PageStateByFilterMap,
    action: TransactionBlocksForAddressActionType,
): PageStateByFilterMap {
    switch (action.type) {
        case PageAction.Next:
            return {
                ...state,
                [action.filterValue]: state[action.filterValue] + 1,
            };
        case PageAction.Prev:
            return {
                ...state,
                [action.filterValue]: state[action.filterValue] - 1,
            };
        case PageAction.First:
            return {
                ...state,
                [action.filterValue]: 0,
            };
        default:
            return { ...state };
    }
}

interface FiltersControlProps {
    filterValue: string;
    setFilterValue: Dispatch<SetStateAction<ObjectFilterValue>>;
}

export function FiltersControl({ filterValue, setFilterValue }: FiltersControlProps): JSX.Element {
    return (
        <RadioGroup
            aria-label="transaction filter"
            value={filterValue}
            onValueChange={(value) => setFilterValue(value as ObjectFilterValue)}
        >
            {FILTER_OPTIONS.map((filter) => (
                <RadioGroupItem key={filter.value} value={filter.value} label={filter.label} />
            ))}
        </RadioGroup>
    );
}

export function TransactionBlocksForAddress({
    address,
    filter = ObjectFilterValue.Changed,
    header,
}: TransactionBlocksForAddressProps): JSX.Element {
    const [filterValue, setFilterValue] = useState(filter);
    const [currentPageState, dispatch] = useReducer(reducer, {
        [ObjectFilterValue.Input]: 0,
        [ObjectFilterValue.Changed]: 0,
    });

    const { data, isPending, isFetching, isFetchingNextPage, fetchNextPage, hasNextPage } =
        useGetTransactionBlocks({
            [filterValue]: address,
        } as TransactionFilter);

    const currentPage = currentPageState[filterValue];
    const cardData =
        data && data.pages[currentPage]
            ? genTableDataFromTxData(data.pages[currentPage].data)
            : undefined;

    return (
        <div data-testid="tx">
            <div className="flex items-center justify-between border-b border-gray-45 pb-5">
                {header && (
                    <Heading color="gray-90" variant="heading4/semibold">
                        {header}
                    </Heading>
                )}

                <FiltersControl filterValue={filterValue} setFilterValue={setFilterValue} />
            </div>

            <div className={clsx(header && 'pt-5', 'flex flex-col space-y-5 text-left xl:pr-10')}>
                {isPending || isFetching || isFetchingNextPage || !cardData ? (
                    <PlaceholderTable
                        rowCount={DEFAULT_TRANSACTIONS_LIMIT}
                        rowHeight="16px"
                        colHeadings={['Digest', 'Sender', 'Txns', 'Gas', 'Time']}
                    />
                ) : (
                    <div>
                        <TableCard data={cardData.data} columns={cardData.columns} />
                    </div>
                )}

                {(hasNextPage || (data && data?.pages.length > 1)) && (
                    <Pagination
                        hasFirst={currentPageState[filterValue] !== 0}
                        onNext={() => {
                            if (isPending || isFetching) {
                                return;
                            }

                            // Make sure we are at the end before fetching another page
                            if (
                                data &&
                                currentPageState[filterValue] === data?.pages.length - 1 &&
                                !isPending &&
                                !isFetching
                            ) {
                                fetchNextPage();
                            }
                            dispatch({
                                type: PageAction.Next,

                                filterValue,
                            });
                        }}
                        hasNext={
                            (Boolean(hasNextPage) && Boolean(data?.pages[currentPage])) ||
                            currentPage < (data?.pages.length ?? 0) - 1
                        }
                        hasPrev={currentPageState[filterValue] !== 0}
                        onPrev={() =>
                            dispatch({
                                type: PageAction.Prev,

                                filterValue,
                            })
                        }
                        onFirst={() =>
                            dispatch({
                                type: PageAction.First,
                                filterValue,
                            })
                        }
                    />
                )}
            </div>
        </div>
    );
}
