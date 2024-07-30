// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { entropyToSerialized, mnemonicToEntropy } from '_src/shared/utils';
import { useNavigate } from 'react-router-dom';

import {
    AccountsFormType,
    useAccountsFormContext,
} from '../../components/accounts/AccountsFormContext';
import { ImportRecoveryPhraseForm } from '../../components/accounts/ImportRecoveryPhraseForm';
import { Header } from '@iota/apps-ui-kit';

export function ImportPassphrasePage() {
    const navigate = useNavigate();
    const [, setFormValues] = useAccountsFormContext();
    return (
        <>
            <Header
                title="Import Mnemonic"
                titleCentered
                hasLeftIcon
                onBack={() =>
                    navigate(
                        `/accounts/add-account?${new URLSearchParams({
                            sourceFlow: 'Onboarding',
                        }).toString()}`,
                    )
                }
            />

            <div className="flex flex-col items-center overflow-auto bg-neutral-100">
                <div className="px-md py-xs">
                    <div className="flex flex-col items-center justify-center gap-y-2 p-md text-center">
                        <p className="text-label-lg text-neutral-40">Wallet Setup</p>
                        <h2 className="text-headline-md text-neutral-10">Add Existing Account</h2>
                        <p className="text-body-md text-neutral-40">
                            Enter your 24-word Recovery Phrase
                        </p>
                    </div>
                </div>

                <div className="flex grow flex-col gap-3 p-md">
                    <ImportRecoveryPhraseForm
                        cancelButtonText="Cancel"
                        submitButtonText="Add Account"
                        onSubmit={({ recoveryPhrase }) => {
                            setFormValues({
                                type: AccountsFormType.ImportMnemonic,
                                entropy: entropyToSerialized(
                                    mnemonicToEntropy(recoveryPhrase.join(' ')),
                                ),
                            });
                            navigate(
                                `/accounts/protect-account?${new URLSearchParams({
                                    accountsFormType: AccountsFormType.ImportMnemonic,
                                }).toString()}`,
                            );
                        }}
                    />
                </div>

                <div className="px-md"></div>
            </div>
        </>
    );
}
