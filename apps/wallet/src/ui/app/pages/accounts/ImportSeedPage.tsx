// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';
import {
    AccountsFormType,
    useAccountsFormContext,
} from '../../components/accounts/AccountsFormContext';
import { ImportSeedForm } from '../../components/accounts/ImportSeedForm';
import { PageTemplate } from '../../components/PageTemplate';

export function ImportSeedPage() {
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();

    return (
        <PageTemplate title="Import Seed" isTitleCentered showBackButton>
            <div className="flex h-full w-full flex-col items-center ">
                <div className="w-full grow">
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
        </PageTemplate>
    );
}
