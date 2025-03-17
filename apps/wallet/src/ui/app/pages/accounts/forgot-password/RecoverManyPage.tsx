// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { entropyToSerialized, mnemonicToEntropy } from '_src/shared/utils';
import {
    ImportRecoveryPhraseForm,
    Overlay,
    RecoverAccountsGroup,
    ImportSeedForm,
    PageTemplate,
} from '_components';
import { useRecoveryDataMutation, useAccountGroups, useAccountSources } from '_hooks';
import { useEffect, useState } from 'react';
import { toast } from '@iota/core';
import { useNavigate } from 'react-router-dom';
import { useForgotPasswordContext } from './ForgotPasswordPage';
import { AccountSourceType } from '_src/background/account-sources/accountSource';
import { AccountType } from '_src/background/accounts/account';
import { Button, ButtonType } from '@iota/apps-ui-kit';

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

    function handleCancel() {
        navigate('/');
    }

    function handleNext() {
        navigate('../warning');
    }

    return (
        <PageTemplate title="Forgot your Passwords?" isTitleCentered onClose={handleCancel}>
            <div className="flex h-full flex-col gap-lg overflow-auto">
                <span className="text-center text-label-lg text-neutral-40 dark:text-neutral-60">
                    Recover the following accounts by completing the recovery process
                </span>
                <div className="flex w-full flex-1 flex-col gap-lg overflow-auto">
                    {mnemonicAccounts.length > 0
                        ? mnemonicAccounts.map(([sourceID, accounts], index) => {
                              const recoveryData = value.find(
                                  ({ accountSourceID }) => accountSourceID === sourceID,
                              );
                              const title = `Mnemonic ${index + 1}`;
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
                          })
                        : null}
                    {seedAccounts.length > 0
                        ? seedAccounts.map(([sourceID, accounts], index) => {
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
                          })
                        : null}
                </div>
                <div className="flex w-full gap-xs">
                    <Button
                        type={ButtonType.Secondary}
                        text="Cancel"
                        onClick={handleCancel}
                        fullWidth
                    />
                    <Button text="Next" disabled={!value.length} onClick={handleNext} fullWidth />
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
                    background="bg-neutral-100 dark:bg-neutral-6"
                >
                    <div className="flex h-full w-full flex-col flex-nowrap gap-4 text-center">
                        {recoverInfo?.type === AccountSourceType.Mnemonic ? (
                            <>
                                <span className="text-label-lg text-neutral-40 dark:text-neutral-60">
                                    Enter your mnemonic to recover your account
                                </span>
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
                                        toast.error(
                                            (e as Error)?.message || 'Something went wrong',
                                        );
                                    }
                                }}
                            />
                        )}
                    </div>
                </Overlay>
            </div>
        </PageTemplate>
    );
}
