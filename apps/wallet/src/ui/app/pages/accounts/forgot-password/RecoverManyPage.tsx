// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { entropyToSerialized, mnemonicToEntropy } from '_src/shared/utils';
import { ImportRecoveryPhraseForm } from '_src/ui/app/components/accounts/ImportRecoveryPhraseForm';
import Overlay from '_src/ui/app/components/overlay';
import { useRecoveryDataMutation } from '_src/ui/app/hooks/useRecoveryDataMutation';
import { useEffect, useState } from 'react';
import toast from 'react-hot-toast';
import { useNavigate } from 'react-router-dom';

import { RecoverAccountsGroup } from '_components/accounts/RecoverAccountsGroup';
import { useAccountGroups } from '../../../hooks/useAccountGroups';
import { useAccountSources } from '../../../hooks/useAccountSources';
import { Button } from '../../../shared/ButtonUI';
import { Heading } from '../../../shared/heading';
import { Text } from '../../../shared/text';
import { useForgotPasswordContext } from './ForgotPasswordPage';
import { ImportSeedForm } from '_src/ui/app/components/accounts/ImportSeedForm';
import { AccountSourceType } from '_src/background/account-sources/AccountSource';
import { AccountType } from '_src/background/accounts/Account';

export function RecoverManyPage() {
    const allAccountSources = useAccountSources();
    const accountGroups = useAccountGroups();
    const mnemonicAccounts = Object.entries(accountGroups[AccountType.MnemonicDerived]);
    const seedAccounts = Object.entries(accountGroups[AccountType.SeedDerived]);
    const navigate = useNavigate();
    const hasMnemonicOrSeedAccountSources = allAccountSources.data?.some(({ type }) =>
        [AccountSourceType.Mnemonic, AccountSourceType.Seed].includes(type),
    );
    useEffect(() => {
        if (!allAccountSources.isPending && !hasMnemonicOrSeedAccountSources) {
            navigate('/', { replace: true });
        }
    }, [allAccountSources.isPending, hasMnemonicOrSeedAccountSources, navigate]);
    const { value } = useForgotPasswordContext();
    const addRecoveryDataMutation = useRecoveryDataMutation();
    const [recoverInfo, setRecoverInfo] = useState<{
        type: AccountSourceType;
        title: string;
        accountSourceID: string;
    } | null>(null);
    return (
        <>
            <div className="flex h-full flex-col items-center gap-6">
                <div className="flex flex-col items-center gap-2 text-center">
                    <Heading variant="heading1" color="gray-90" as="h1" weight="bold">
                        Forgot Password?
                    </Heading>
                    <Text variant="pBody" color="gray-90">
                        Please complete the recovery process for the accounts below
                    </Text>
                </div>
                {mnemonicAccounts.length > 0 ? (
                    <div className="bg-hero-darkest/5 flex grow flex-col gap-8 self-stretch overflow-y-auto overflow-x-hidden rounded-lg px-4 py-6">
                        {mnemonicAccounts.map(([sourceID, accounts], index) => {
                            const recoveryData = value.find(
                                ({ accountSourceID }) => accountSourceID === sourceID,
                            );
                            const title = `Passphrase ${index + 1}`;
                            return (
                                <RecoverAccountsGroup
                                    key={sourceID}
                                    title={title}
                                    accounts={accounts}
                                    showRecover={!recoveryData}
                                    onRecover={() => {
                                        setRecoverInfo({
                                            title,
                                            accountSourceID: sourceID,
                                            type: AccountSourceType.Mnemonic,
                                        });
                                    }}
                                    recoverDone={!!recoveryData}
                                />
                            );
                        })}
                    </div>
                ) : null}
                {seedAccounts.length > 0 ? (
                    <div className="bg-hero-darkest/5 flex grow flex-col gap-8 self-stretch overflow-y-auto overflow-x-hidden rounded-lg px-4 py-6">
                        {seedAccounts.map(([sourceID, accounts], index) => {
                            const recoveryData = value.find(
                                ({ accountSourceID }) => accountSourceID === sourceID,
                            );
                            const title = `Seed ${index + 1}`;
                            return (
                                <RecoverAccountsGroup
                                    key={sourceID}
                                    title={title}
                                    accounts={accounts}
                                    showRecover={!recoveryData}
                                    onRecover={() => {
                                        setRecoverInfo({
                                            title,
                                            accountSourceID: sourceID,
                                            type: AccountSourceType.Seed,
                                        });
                                    }}
                                    recoverDone={!!recoveryData}
                                />
                            );
                        })}
                    </div>
                ) : null}
                <div className="flex w-full flex-nowrap gap-2.5">
                    <Button variant="outline" size="tall" text="Cancel" to="/" />
                    <Button
                        variant="primary"
                        size="tall"
                        text="Next"
                        disabled={!value.length}
                        to="../warning"
                    />
                </div>
            </div>
            <Overlay
                title={recoverInfo?.title}
                showModal={!!recoverInfo}
                closeOverlay={() => {
                    if (addRecoveryDataMutation.isPending) {
                        return;
                    }
                    setRecoverInfo(null);
                }}
                background="bg-iota-lightest"
            >
                <div className="flex h-full w-full flex-col flex-nowrap gap-4 text-center">
                    {recoverInfo?.type === AccountSourceType.Mnemonic ? (
                        <>
                            <Text variant="pBody" color="gray-90">
                                Enter your 24-word Recovery Phrase
                            </Text>
                            <ImportRecoveryPhraseForm
                                submitButtonText="Recover"
                                onSubmit={async ({ recoveryPhrase }) => {
                                    if (!recoverInfo) {
                                        return;
                                    }
                                    try {
                                        await addRecoveryDataMutation.mutateAsync({
                                            type: AccountSourceType.Mnemonic,
                                            entropy: entropyToSerialized(
                                                mnemonicToEntropy(recoveryPhrase.join(' ')),
                                            ),
                                            accountSourceID: recoverInfo.accountSourceID,
                                        });
                                        setRecoverInfo(null);
                                    } catch (e) {
                                        toast.error(
                                            (e as Error)?.message || 'Something went wrong',
                                        );
                                    }
                                }}
                            />
                        </>
                    ) : (
                        <ImportSeedForm
                            onSubmit={async ({ seed }) => {
                                if (!recoverInfo) {
                                    return;
                                }
                                try {
                                    await addRecoveryDataMutation.mutateAsync({
                                        type: AccountSourceType.Seed,
                                        accountSourceID: recoverInfo.accountSourceID,
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
            </Overlay>
        </>
    );
}
