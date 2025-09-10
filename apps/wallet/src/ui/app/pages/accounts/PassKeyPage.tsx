// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';

import { AccountsFormType, PageTemplate, useAccountsFormContext } from '_components';

export function PassKeyPage() {
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();

    function handleOnSubmit() {
        setAccountsFormValues({
            type: AccountsFormType.Passkey,
        });
        navigate(
            `/accounts/protect-account?${new URLSearchParams({
                accountsFormType: AccountsFormType.Passkey,
            }).toString()}`,
        );
    }

    return (
        <PageTemplate title="PassKey" isTitleCentered showBackButton>
            <div className="flex h-full w-full flex-col items-center ">
                <button
                    onClick={() => handleOnSubmit()}
                    className="mt-4 rounded bg-blue-500 px-4 py-2 text-white hover:bg-blue-600"
                >
                    Submit
                </button>
            </div>
        </PageTemplate>
    );
}
