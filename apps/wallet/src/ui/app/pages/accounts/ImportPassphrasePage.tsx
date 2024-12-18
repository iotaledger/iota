// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { entropyToSerialized, mnemonicToEntropy } from '_src/shared/utils/bip39';
import { useNavigate } from 'react-router-dom';

import { useAccountsFormContext } from '../../components/accounts/AccountsFormContext';
import { ImportRecoveryPhraseForm } from '../../components/accounts/ImportRecoveryPhraseForm';
import { Heading } from '../../shared/heading';

export function ImportPassphrasePage() {
    const navigate = useNavigate();
    const [, setFormValues] = useAccountsFormContext();
    return (
        <div className="flex h-full flex-col items-center overflow-auto rounded-20 bg-iota-lightest px-6 py-10 shadow-wallet-content">
            <Text variant="caption" color="steel-dark" weight="semibold">
                Wallet Setup
            </Text>
            <div className="mt-2.5 text-center">
                <Heading variant="heading1" color="gray-90" as="h1" weight="bold">
                    Add Existing Account
                </Heading>
            </div>
            <div className="mt-6 flex grow flex-col gap-3">
                <div className="pl-2.5">
                    <Text variant="pBody" color="steel-darker" weight="semibold">
                        Enter your 12-word Recovery Phrase
                    </Text>
                </div>
                <ImportRecoveryPhraseForm
                    cancelButtonText="Cancel"
                    submitButtonText="Add Account"
                    onSubmit={({ recoveryPhrase }) => {
                        setFormValues({
                            type: 'import-mnemonic',
                            entropy: entropyToSerialized(
                                mnemonicToEntropy(recoveryPhrase.join(' ')),
                            ),
                        });
                        navigate('/accounts/protect-account?accountType=import-mnemonic');
                    }}
                />
            </div>
        </div>
    );
}
