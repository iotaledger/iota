// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useTransactionData, useTransactionGasBudget } from '_src/ui/app/hooks';
import { GAS_SYMBOL } from '_src/ui/app/redux/slices/iota-objects/Coin';
import { type Transaction } from '@iota/iota-sdk/transactions';
import { formatAddress } from '@iota/iota-sdk/utils';

import { DescriptionItem, DescriptionList } from './DescriptionList';
import { SummaryCard } from './SummaryCard';

interface Props {
	sender?: string;
	transaction: Transaction;
}

export function GasFees({ sender, transaction }: Props) {
	const { data: transactionData } = useTransactionData(sender, transaction);
	const { data: gasBudget, isPending, isError } = useTransactionGasBudget(sender, transaction);
	const isSponsored =
		transactionData?.gasData.owner && transactionData.sender !== transactionData.gasData.owner;
	return (
		<SummaryCard
			header="Estimated Gas Fees"
			badge={
				isSponsored ? (
					<div className="bg-white text-success px-1.5 py-0.5 text-captionSmallExtra rounded-full font-medium uppercase">
						Sponsored
					</div>
				) : null
			}
			initialExpanded
		>
			<DescriptionList>
				<DescriptionItem title="You Pay">
					{isPending
						? 'Estimating...'
						: isError
							? 'Gas estimation failed'
							: `${isSponsored ? 0 : gasBudget} ${GAS_SYMBOL}`}
				</DescriptionItem>
				{isSponsored && (
					<>
						<DescriptionItem title="Sponsor Pays">
							{gasBudget ? `${gasBudget} ${GAS_SYMBOL}` : '-'}
						</DescriptionItem>
						<DescriptionItem title="Sponsor">
							{formatAddress(transactionData!.gasData.owner!)}
						</DescriptionItem>
					</>
				)}
			</DescriptionList>
		</SummaryCard>
	);
}
