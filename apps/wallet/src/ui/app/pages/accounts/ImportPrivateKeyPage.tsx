// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate, useSearchParams } from 'react-router-dom';
import { AmpliSourceFlow } from '_src/shared/analytics';

import {
    AccountsFormType,
    ImportPrivateKeyForm,
    PageTemplate,
    useAccountsFormContext,
} from '_components';

export function ImportPrivateKeyPage() {
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const sourceFlow = searchParams.get('sourceFlow') || AmpliSourceFlow.Unknown;
    const [, setAccountsFormValues] = useAccountsFormContext();

    function handleOnSubmit({ privateKey }: { privateKey: string }) {
        setAccountsFormValues({
            type: AccountsFormType.ImportPrivateKey,
            keyPair: privateKey,
        });
        navigate(
            `/accounts/protect-account?${new URLSearchParams({
                accountsFormType: AccountsFormType.ImportPrivateKey,
                sourceFlow,
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
