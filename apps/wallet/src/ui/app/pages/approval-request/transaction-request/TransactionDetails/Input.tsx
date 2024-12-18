// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import ExplorerLink from '_src/ui/app/components/explorer-link';
import { ExplorerLinkType } from '_src/ui/app/components/explorer-link/ExplorerLinkType';
import { Text } from '_src/ui/app/shared/text';
import { type TransactionInput } from '@iota/iota-sdk/transactions';
import { formatAddress } from '@iota/iota-sdk/utils';

interface InputProps {
	input: TransactionInput;
}

export function Input({ input }: InputProps) {
	const { objectId } =
		input?.Object?.ImmOrOwnedObject ??
		input?.Object?.SharedObject ??
		input.Object?.Receiving! ??
		{};

	return (
		<div className="break-all">
			<Text variant="pBodySmall" weight="medium" color="steel-dark" mono>
				{input.Pure ? (
					`${input.Pure.bytes}`
				) : input.Object ? (
					<ExplorerLink type={ExplorerLinkType.object} objectID={objectId!}>
						{formatAddress(objectId)}
					</ExplorerLink>
				) : (
					'Unknown input value'
				)}
			</Text>
		</div>
	);
}
