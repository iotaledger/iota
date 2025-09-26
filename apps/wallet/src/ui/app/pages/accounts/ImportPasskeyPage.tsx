// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';

import { AccountsFormType, PageTemplate, useAccountsFormContext } from '_components';
import { Button, ButtonHtmlType, ButtonType } from '@iota/apps-ui-kit';
import { useState } from 'react';

export function ImportPasskeyPage() {
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();
    const [authenticatorType, setAuthenticatorType] = useState<AuthenticatorAttachment>('platform');

    function handleOnSubmit() {
        setAccountsFormValues({
            type: AccountsFormType.Passkey,
            authenticatorAttachment: authenticatorType,
        });
        navigate(
            `/accounts/protect-account?${new URLSearchParams({
                accountsFormType: AccountsFormType.Passkey,
            }).toString()}`,
        );
    }

    return (
        <PageTemplate title="PassKey" isTitleCentered showBackButton>
            <div className="mb-4 text-center">
                <p className="text-title-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                    Selected Authenticator Type:
                </p>
                <div className="text-title-sm capitalize text-iota-neutral-10 dark:text-iota-neutral-92">
                    {authenticatorType}
                </div>
            </div>
            <div className="mt-auto flex gap-xs pb-md pt-xs">
                <Button
                    fullWidth
                    text="Platform"
                    onClick={() => setAuthenticatorType('platform')}
                    type={ButtonType.Secondary}
                />
                <Button
                    fullWidth
                    text="Cross-Platform"
                    onClick={() => setAuthenticatorType('cross-platform')}
                    type={ButtonType.Secondary}
                />
            </div>
            <div className="flex h-full w-full flex-col items-center ">
                <Button
                    htmlType={ButtonHtmlType.Submit}
                    fullWidth
                    onClick={() => handleOnSubmit()}
                    type={ButtonType.Primary}
                    text="Add Account"
                />
            </div>
        </PageTemplate>
    );
}
