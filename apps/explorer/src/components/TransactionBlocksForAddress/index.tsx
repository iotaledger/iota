// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { type TransactionFilter } from '@mysten/sui.js/client';
import { Heading, RadioGroup, RadioGroupItem } from '@mysten/ui';
import { useReducer, useState } from 'react';

import { genTableDataFromTxData } from '../transactions/TxCardUtils';
import {
	DEFAULT_TRANSACTIONS_LIMIT,
	useGetTransactionBlocks,
} from '~/hooks/useGetTransactionBlocks';
import { Pagination } from '~/ui/Pagination';
import { PlaceholderTable } from '~/ui/PlaceholderTable';
import { TableCard } from '~/ui/TableCard';
import clsx from 'clsx';

export enum ObjectFilterValue {
	Input = 'inputObject',
	Changed = 'changedObject',
}

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

const reducer = (state: PageStateByFilterMap, action: TransactionBlocksForAddressActionType) => {
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
};

export function FiltersControl({
	filterValue,
	setFilterValue,
}: {
	filterValue: string;
	setFilterValue: any;
}) {
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

function TransactionBlocksForAddress({
	address,
	filter = ObjectFilterValue.Changed,
	header,
}: TransactionBlocksForAddressProps) {
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
						colWidths={['30%', '30%', '10%', '20%', '10%']}
					/>
				) : (
					<div>
						<TableCard data={cardData.data} columns={cardData.columns} />
					</div>
				)}

				{(hasNextPage || (data && data?.pages.length > 1)) && (
					<Pagination
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

export default TransactionBlocksForAddress;
