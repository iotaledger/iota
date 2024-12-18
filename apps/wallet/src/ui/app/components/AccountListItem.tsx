// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useResolveIotaNSName } from '_app/hooks/useAppResolveIotaNSName';
import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { Check12, Copy12 } from '@iota/icons';
import { formatAddress } from '@iota/iota-sdk/utils';

import { useCopyToClipboard } from '../hooks/useCopyToClipboard';
import { Text } from '../shared/text';
import { AccountBadge } from './AccountBadge';

export type AccountItemProps = {
	account: SerializedUIAccount;
	onAccountSelected: (account: SerializedUIAccount) => void;
};

/** @deprecated - use AccountListItem from the `accounts` folder **/
export function AccountListItem({ account, onAccountSelected }: AccountItemProps) {
	const { address, type, selected } = account;
	const copy = useCopyToClipboard(address, {
		copySuccessMessage: 'Address Copied',
	});
	const domainName = useResolveIotaNSName(address);

	return (
		<li>
			<button
				className="appearance-none bg-transparent border-0 w-full flex p-2.5 items-center gap-2.5 rounded-md hover:bg-iota/10 cursor-pointer focus-visible:ring-1 group transition-colors text-left"
				onClick={() => {
					onAccountSelected(account);
				}}
			>
				<div className="flex items-center gap-2 flex-1 min-w-0">
					<div className="min-w-0">
						<Text color="steel-darker" variant="bodySmall" truncate mono>
							{domainName ?? formatAddress(address)}
						</Text>
					</div>
					<AccountBadge accountType={type} />
				</div>
				{selected ? <Check12 className="text-success" /> : null}
				<Copy12
					className="text-gray-60 group-hover:text-steel transition-colors hover:!text-hero-dark"
					onClick={copy}
				/>
			</button>
		</li>
	);
}
