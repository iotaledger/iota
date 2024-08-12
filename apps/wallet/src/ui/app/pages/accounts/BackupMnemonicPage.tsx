// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button } from '_app/shared/ButtonUI';
import { CardLayout } from '_app/shared/card-layout';
import { Text } from '_app/shared/text';
import { Alert, Loading, HideShowDisplayBox, VerifyPasswordModal } from '_components';
import { ArrowLeft16, Check12 } from '@iota/icons';
import { useEffect, useMemo, useState } from 'react';
import { Navigate, useLocation, useNavigate, useParams } from 'react-router-dom';

import { useAccountSources } from '../../hooks/useAccountSources';
import { useExportPassphraseMutation } from '../../hooks/useExportPassphraseMutation';
import { AccountSourceType } from '_src/background/account-sources/AccountSource';

export function BackupMnemonicPage() {
    const [passwordCopied, setPasswordCopied] = useState(false);
    const { state } = useLocation();
    const { accountSourceID } = useParams();
    const { data: accountSources, isPending } = useAccountSources();
    const selectedSource = useMemo(
        () => accountSources?.find(({ id }) => accountSourceID === id),
        [accountSources, accountSourceID],
    );
    const isOnboardingFlow = !!state?.onboarding;
    const [showPasswordDialog, setShowPasswordDialog] = useState(false);
    const [passwordConfirmed, setPasswordConfirmed] = useState(false);
    const requirePassword = !isOnboardingFlow || !!selectedSource?.isLocked;
    const passphraseMutation = useExportPassphraseMutation();
    useEffect(() => {
        (async () => {
            if (
                (requirePassword && !passwordConfirmed) ||
                !passphraseMutation.isIdle ||
                !accountSourceID
            ) {
                return;
            }
            passphraseMutation.mutate({ accountSourceID: accountSourceID });
        })();
    }, [requirePassword, passwordConfirmed, accountSourceID, passphraseMutation]);
    useEffect(() => {
        if (requirePassword && !passwordConfirmed && !showPasswordDialog) {
            setShowPasswordDialog(true);
        }
    }, [requirePassword, passwordConfirmed, showPasswordDialog]);
    const navigate = useNavigate();
    if (!isPending && selectedSource?.type !== AccountSourceType.Mnemonic) {
        return <Navigate to="/" replace />;
    }
    return (
        <Loading loading={isPending}>
            {showPasswordDialog ? (
                <CardLayout>
                    <VerifyPasswordModal
                        open
                        onClose={() => {
                            navigate(-1);
                        }}
                        onVerify={async (password) => {
                            await passphraseMutation.mutateAsync({
                                password,
                                accountSourceID: selectedSource!.id,
                            });
                            setPasswordConfirmed(true);
                            setShowPasswordDialog(false);
                        }}
                    />
                </CardLayout>
            ) : (
                <CardLayout
                    icon={isOnboardingFlow ? 'success' : undefined}
                    title={
                        isOnboardingFlow ? 'Wallet Created Successfully!' : 'Backup Recovery Phrase'
                    }
                >
                    <div className="flex h-full w-full flex-grow flex-col flex-nowrap">
                        <div className="mb-5 flex flex-grow flex-col flex-nowrap">
                            <div className="mb-1 mt-7.5 text-center">
                                <Text variant="caption" color="steel-darker" weight="bold">
                                    Recovery phrase
                                </Text>
                            </div>
                            <div className="mb-3.5 mt-2 text-center">
                                <Text variant="pBodySmall" color="steel-dark" weight="normal">
                                    Your recovery phrase makes it easy to back up and restore your
                                    account.
                                </Text>
                            </div>
                            <Loading loading={passphraseMutation.isPending}>
                                {passphraseMutation.data ? (
                                    <HideShowDisplayBox value={passphraseMutation.data} hideCopy />
                                ) : (
                                    <Alert>
                                        {(passphraseMutation.error as Error)?.message ||
                                            'Something went wrong'}
                                    </Alert>
                                )}
                            </Loading>
                            <div className="mb-1 mt-3.75 text-center">
                                <Text variant="caption" color="steel-dark" weight="semibold">
                                    Warning
                                </Text>
                            </div>
                            <div className="mb-1 text-center">
                                <Text variant="pBodySmall" color="steel-dark" weight="normal">
                                    Never disclose your secret recovery phrase. Anyone can take over
                                    your account with it.
                                </Text>
                            </div>
                            <div className="flex-1" />
                            {isOnboardingFlow ? (
                                <div className="mb- mt-5 flex w-full text-left">
                                    <label className="text-iota-dark relative mb-0 mr-5 flex h-5 cursor-pointer items-center justify-center gap-1.25">
                                        <input
                                            type="checkbox"
                                            name="agree"
                                            id="agree"
                                            className="peer/agree invisible ml-2"
                                            onChange={() => setPasswordCopied(!passwordCopied)}
                                        />
                                        <span className="peer-checked/agree:bg-success absolute left-0 top-0 flex h-5 w-5 items-center justify-center rounded border border-gray-50 bg-white shadow-button peer-checked/agree:shadow-none">
                                            <Check12 className="text-body font-semibold text-white" />
                                        </span>

                                        <Text
                                            variant="bodySmall"
                                            color="steel-dark"
                                            weight="normal"
                                        >
                                            I saved my recovery phrase
                                        </Text>
                                    </label>
                                </div>
                            ) : null}
                        </div>
                        <Button
                            type="button"
                            size="tall"
                            variant="primary"
                            disabled={!passwordCopied && isOnboardingFlow}
                            to="/"
                            text="Open Iota Wallet"
                            after={
                                <ArrowLeft16 className="rotate-135 text-pBodySmall font-normal" />
                            }
                        />
                    </div>
                </CardLayout>
            )}
        </Loading>
    );
}
