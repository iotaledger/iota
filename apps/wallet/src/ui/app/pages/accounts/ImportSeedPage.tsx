// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate, useSearchParams } from 'react-router-dom';
import { AmpliSourceFlow } from '_src/shared/analytics';
import {
    AccountsFormType,
    useAccountsFormContext,
    ImportSeedForm,
    PageTemplate,
} from '_components';

export function ImportSeedPage() {
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const sourceFlow = searchParams.get('sourceFlow') || AmpliSourceFlow.Unknown;
    const [, setAccountsFormValues] = useAccountsFormContext();

    function handleOnSubmit({ seed }: { seed: string }) {
        setAccountsFormValues({
            type: AccountsFormType.ImportSeed,
            seed,
        });
        navigate(
            `/accounts/protect-account?${new URLSearchParams({
                accountsFormType: AccountsFormType.ImportSeed,
                sourceFlow,
            }).toString()}`,
        );
    }

    return (
        <PageTemplate title="Import Seed" isTitleCentered showBackButton>
            <div className="flex h-full w-full flex-col items-center ">
                <div className="w-full grow">
                    <ImportSeedForm onSubmit={handleOnSubmit} />
                </div>
            </div>
        </PageTemplate>
    );
}
