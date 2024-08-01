// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { useNavigate } from 'react-router-dom';

import {
    AccountsFormType,
    useAccountsFormContext,
} from '../../components/accounts/AccountsFormContext';
import { ImportSeedForm } from '../../components/accounts/ImportSeedForm';
import { Heading } from '../../shared/heading';

export function ImportSeedPage() {
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();

    return (
        <div className="bg-iota-lightest flex h-full w-full flex-col items-center rounded-20 px-6 py-10 shadow-wallet-content">
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
                            type: AccountsFormType.ImportSeed,
                            seed,
                        });
                        navigate(
                            `/accounts/protect-account?${new URLSearchParams({
                                accountsFormType: AccountsFormType.ImportSeed,
                            }).toString()}`,
                        );
                    }}
                />
            </div>
        </div>
    );
}
