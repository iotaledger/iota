// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { Heading } from '_src/ui/app/shared/heading';
import { Text } from '_src/ui/app/shared/text';
import { useFormatCoin } from '@iota/core';

import { Card } from '../Card';

export function TotalAmount({ amount, coinType }: { amount?: string; coinType?: string }) {
	const [formatted, symbol] = useFormatCoin(amount, coinType);
	if (!amount) return null;
	return (
		<Card>
			<div className="flex justify-between items-center">
				<Text color="steel-darker" variant="pBody">
					Total Amount
				</Text>
				<div className="flex gap-0.5 items-center">
					<Heading color="steel-darker" variant="heading2">
						{formatted}
					</Heading>
					<Text color="steel-darker" variant="body" weight="medium">
						{symbol}
					</Text>
				</div>
			</div>
		</Card>
	);
}
