// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';
import {
    AccountsFormType,
    useAccountsFormContext,
} from '../../components/accounts/AccountsFormContext';
import { ImportSeedForm } from '../../components/accounts/ImportSeedForm';
import { Header } from '@iota/apps-ui-kit';

export function ImportSeedPage() {
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();

    return (
        <div className="flex h-full w-full flex-col bg-white">
            <Header title="Import Seed" titleCentered onBack={() => navigate(-1)} />
            <div className="flex h-full flex-col gap-4 p-md">
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
