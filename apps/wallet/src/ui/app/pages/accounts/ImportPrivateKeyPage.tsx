// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';

import {
    AccountsFormType,
    useAccountsFormContext,
} from '../../components/accounts/AccountsFormContext';
import { ImportPrivateKeyForm } from '../../components/accounts/ImportPrivateKeyForm';
import { Header } from '@iota/apps-ui-kit';

export function ImportPrivateKeyPage() {
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();

    return (
        <>
            <Header
                title="Import Private Key"
                onBack={() => {
                    navigate(-1);
                }}
            />
            <div className="flex h-full w-full flex-col items-center bg-neutral-100 p-md">
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
        </>
    );
}
