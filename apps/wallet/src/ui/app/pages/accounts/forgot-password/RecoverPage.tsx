// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { entropyToSerialized, mnemonicToEntropy } from '_src/shared/utils';
import { ImportRecoveryPhraseForm, ImportSeedForm } from '_components';
import { useRecoveryDataMutation } from '_src/ui/app/hooks/useRecoveryDataMutation';
import { useEffect } from 'react';
import toast from 'react-hot-toast';
import { useNavigate } from 'react-router-dom';

import { useAccountSources } from '../../../hooks/useAccountSources';
import { Heading } from '../../../shared/heading';
import { Text } from '../../../shared/text';
import { AccountSourceType } from '_src/background/account-sources/AccountSource';

export function RecoverPage() {
    const allAccountSources = useAccountSources();
    const navigate = useNavigate();
    const mnemonicAccountSource = allAccountSources.data?.find(
        ({ type }) => type === AccountSourceType.Mnemonic,
    );
    const seedAccountSource = allAccountSources.data?.find(
        ({ type }) => type === AccountSourceType.Seed,
    );
    useEffect(() => {
        if (!allAccountSources.isPending && !mnemonicAccountSource && !seedAccountSource) {
            navigate('/', { replace: true });
        }
    }, [allAccountSources.isPending, mnemonicAccountSource, seedAccountSource, navigate]);
    const recoveryDataMutation = useRecoveryDataMutation();
    if (!mnemonicAccountSource && !seedAccountSource) {
        return null;
    }
    const title = mnemonicAccountSource
        ? 'Recover with 24-word Recovery Phrase'
        : 'Recover with Seed';
    return (
        <div className="flex w-full flex-1 flex-col items-center gap-6">
            <div className="flex flex-col items-center gap-2">
                <Heading variant="heading1" color="gray-90" as="h1" weight="bold">
                    Forgot Password?
                </Heading>
                <Text variant="pBody" color="gray-90">
                    {title}
                </Text>
            </div>
            <div className="w-full grow">
                {mnemonicAccountSource ? (
                    <ImportRecoveryPhraseForm
                        cancelButtonText="Cancel"
                        submitButtonText="Next"
                        onSubmit={async ({ recoveryPhrase }) => {
                            try {
                                await recoveryDataMutation.mutateAsync({
                                    type: AccountSourceType.Mnemonic,
                                    accountSourceID: mnemonicAccountSource.id,
                                    entropy: entropyToSerialized(
                                        mnemonicToEntropy(recoveryPhrase.join(' ')),
                                    ),
                                });
                                navigate('../warning');
                            } catch (e) {
                                toast.error((e as Error)?.message || 'Something went wrong');
                            }
                        }}
                    />
                ) : (
                    <ImportSeedForm
                        onSubmit={async ({ seed }) => {
                            try {
                                await recoveryDataMutation.mutateAsync({
                                    type: AccountSourceType.Seed,
                                    accountSourceID: seedAccountSource?.id ?? '',
                                    seed,
                                });
                                navigate('../warning');
                            } catch (e) {
                                toast.error((e as Error)?.message || 'Something went wrong');
                            }
                        }}
                    />
                )}
            </div>
        </div>
    );
}
