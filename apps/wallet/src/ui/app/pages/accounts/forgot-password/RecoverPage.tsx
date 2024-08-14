// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { entropyToSerialized, mnemonicToEntropy } from '_src/shared/utils';
import { ImportRecoveryPhraseForm } from '_src/ui/app/components/accounts/ImportRecoveryPhraseForm';
import { useRecoveryDataMutation } from '_src/ui/app/hooks/useRecoveryDataMutation';
import { useEffect } from 'react';
import toast from 'react-hot-toast';
import { useNavigate } from 'react-router-dom';
import { useAccountSources } from '../../../hooks/useAccountSources';
import { ImportSeedForm } from '_src/ui/app/components/accounts/ImportSeedForm';
import { AccountSourceType } from '_src/background/account-sources/AccountSource';
import { PageTemplate } from '_src/ui/app/components/PageTemplate';

export function RecoverPage() {
    const allAccountSources = useAccountSources();
    const navigate = useNavigate();
    const mnemonicAccountSource = allAccountSources.data?.find(
        ({ type }: { type: AccountSourceType }) => type === AccountSourceType.Mnemonic,
    );
    const seedAccountSource = allAccountSources.data?.find(
        ({ type }: { type: AccountSourceType }) => type === AccountSourceType.Seed,
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
    const DESCRIPTION_TEXT = mnemonicAccountSource
        ? 'Recover with 24-word Recovery Phrase'
        : 'Recover with Seed';

    async function handleOnSubmitRecoveryPhrase({ recoveryPhrase }: { recoveryPhrase: string[] }) {
        try {
            await recoveryDataMutation.mutateAsync({
                type: AccountSourceType.Mnemonic,
                accountSourceID: mnemonicAccountSource?.id ?? '',
                entropy: entropyToSerialized(mnemonicToEntropy(recoveryPhrase.join(' '))),
            });
            navigate('../warning');
        } catch (e) {
            toast.error((e as Error)?.message || 'Something went wrong');
        }
    }

    async function handleOnSubmitSeed({ seed }: { seed: string }) {
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
    }

    return (
        <PageTemplate title="Forgot Password?" isTitleCentered showBackButton>
            <div className="flex h-full flex-col gap-md">
                <span className="text-label-lg text-neutral-40">{DESCRIPTION_TEXT}</span>
                <div className="flex h-full flex-col overflow-hidden">
                    {mnemonicAccountSource ? (
                        <ImportRecoveryPhraseForm
                            cancelButtonText="Cancel"
                            submitButtonText="Next"
                            onSubmit={handleOnSubmitRecoveryPhrase}
                        />
                    ) : (
                        <ImportSeedForm onSubmit={handleOnSubmitSeed} />
                    )}
                </div>
            </div>
        </PageTemplate>
    );
}
