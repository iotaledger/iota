// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { useNavigate } from 'react-router-dom';

import { AccountsFormType, useAccountsFormContext } from '_components/accounts/AccountsFormContext';
import { ImportPrivateKeyForm } from '_components/accounts/ImportPrivateKeyForm';
import { Heading } from '_app/shared/heading';

export function ImportPrivateKeyPage() {
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();

    return (
        <div className="bg-iota-lightest flex h-full w-full flex-col items-center rounded-20 px-6 py-10 shadow-wallet-content">
            <Text variant="caption" color="steel-dark" weight="semibold">
                Wallet Setup
            </Text>
            <div className="mt-2.5 text-center">
                <Heading variant="heading1" color="gray-90" as="h1" weight="bold">
                    Import Private Key
                </Heading>
            </div>
            <div className="mt-6 w-full grow">
                <ImportPrivateKeyForm
                    onSubmit={({ privateKey }) => {
                        setAccountsFormValues({
                            type: AccountsFormType.ImportPrivateKey,
                            keyPair: privateKey,
                        });
                        navigate(
                            `/accounts/protect-account?${new URLSearchParams({
                                accountsFormType: AccountsFormType.ImportPrivateKey,
                            }).toString()}`,
                        );
                    }}
                />
            </div>
        </div>
    );
}
