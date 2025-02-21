// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { entropyToSerialized, mnemonicToEntropy } from '_src/shared/utils';
import { useNavigate } from 'react-router-dom';

import {
    AccountsFormType,
    useAccountsFormContext,
    ImportRecoveryPhraseForm,
    PageTemplate,
} from '_components';
import { Button, ButtonType } from '@iota/apps-ui-kit';
import { useState } from 'react';
import { VisibilityOff, VisibilityOn } from '@iota/apps-ui-icons';

export function ImportPassphrasePage() {
    const navigate = useNavigate();
    const [, setFormValues] = useAccountsFormContext();
    const [isTextVisible, setIsTextVisible] = useState(false);

    function handleShowTextClick() {
        setIsTextVisible(!isTextVisible);
    }

    function handleOnSubmit({ recoveryPhrase }: { recoveryPhrase: string[] }) {
        setFormValues({
            type: AccountsFormType.ImportMnemonic,
            entropy: entropyToSerialized(mnemonicToEntropy(recoveryPhrase.join(' '))),
        });
        navigate(
            `/accounts/protect-account?${new URLSearchParams({
                accountsFormType: AccountsFormType.ImportMnemonic,
            }).toString()}`,
        );
    }

    const BUTTON_ICON_CLASSES = 'w-5 h-5 text-neutral-10 dark:text-neutral-92';
    return (
        <PageTemplate title="Import Mnemonic" isTitleCentered showBackButton>
            <div className="flex h-full flex-col gap-md">
                <div className="flex w-full flex-col items-end">
                    <Button
                        text={isTextVisible ? 'Hide Text' : 'Show Text'}
                        icon={
                            isTextVisible ? (
                                <VisibilityOff className={BUTTON_ICON_CLASSES} />
                            ) : (
                                <VisibilityOn className={BUTTON_ICON_CLASSES} />
                            )
                        }
                        onClick={handleShowTextClick}
                        type={ButtonType.Secondary}
                    />
                </div>
                <div className="flex h-full flex-col overflow-hidden">
                    <ImportRecoveryPhraseForm
                        cancelButtonText="Back"
                        submitButtonText="Add Profile"
                        isTextVisible={isTextVisible}
                        onSubmit={handleOnSubmit}
                    />
                </div>
            </div>
        </PageTemplate>
    );
}
