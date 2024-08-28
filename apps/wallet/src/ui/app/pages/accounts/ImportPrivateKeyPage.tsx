// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';

import {
    AccountsFormType,
    ImportPrivateKeyForm,
    PageTemplate,
    useAccountsFormContext,
} from '_components';

export function ImportPrivateKeyPage() {
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();

    function handleOnSubmit({ privateKey }: { privateKey: string }) {
        setAccountsFormValues({
            type: AccountsFormType.ImportPrivateKey,
            keyPair: privateKey,
        });
        navigate(
            `/accounts/protect-account?${new URLSearchParams({
                accountsFormType: AccountsFormType.ImportPrivateKey,
            }).toString()}`,
        );
    }

    return (
        <PageTemplate title="Import Private Key" isTitleCentered showBackButton>
            <div className="flex h-full w-full flex-col items-center ">
                <div className="w-full grow">
                    <ImportPrivateKeyForm onSubmit={handleOnSubmit} />
                </div>
            </div>
        </PageTemplate>
    );
}
