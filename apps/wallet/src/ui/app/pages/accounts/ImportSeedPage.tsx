// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { useNavigate } from 'react-router-dom';

import { useAccountsFormContext } from '../../components/accounts/AccountsFormContext';
import { ImportSeedForm } from '../../components/accounts/ImportSeedForm';
import { Heading } from '../../shared/heading';

export function ImportSeedPage() {
	const navigate = useNavigate();
	const [, setAccountsFormValues] = useAccountsFormContext();

	return (
		<div className="rounded-20 shadow-wallet-content flex h-full w-full flex-col items-center bg-sui-lightest px-6 py-10">
			<Text variant="caption" color="steel-dark" weight="semibold">
				Wallet Setup
			</Text>
			<div className="mt-2.5 text-center">
				<Heading variant="heading1" color="gray-90" as="h1" weight="bold">
					Import Seed
				</Heading>
			</div>
			<div className="mt-6 w-full grow">
				<ImportSeedForm
					onSubmit={({ seed }) => {
						setAccountsFormValues({
							type: 'import-seed',
							seed,
						});
						navigate('/accounts/protect-account?accountType=import-seed');
					}}
				/>
			</div>
		</div>
	);
}
