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
		<div className="rounded-20 bg-sui-lightest shadow-wallet-content flex flex-col items-center px-6 py-10 w-full h-full">
			<Text variant="caption" color="steel-dark" weight="semibold">
				Wallet Setup
			</Text>
			<div className="text-center mt-2.5">
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
