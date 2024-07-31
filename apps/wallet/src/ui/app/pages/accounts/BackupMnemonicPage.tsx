// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// import { Button } from '_app/shared/ButtonUI';
import { CardLayout } from '_app/shared/card-layout';
import Alert from '_components/alert';
import Loading from '_components/loading';
import { ThumbUpFill32 } from '@iota/icons';
import { Button, ButtonType, Checkbox, TextArea } from '@iota/apps-ui-kit';
import { useEffect, useMemo, useState } from 'react';
import { Navigate, useLocation, useNavigate, useParams } from 'react-router-dom';

import { VerifyPasswordModal } from '../../components/accounts/VerifyPasswordModal';
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
                <div className="flex max-h-popup-height w-full max-w-popup-width flex-grow flex-col items-center justify-between gap-4 bg-white p-md text-center">
                    <div className="flex flex-col gap-4">
                        <div className="flex flex-col items-center gap-6 px-md py-sm">
                            {isOnboardingFlow && (
                                <div className="flex w-fit rounded-lg bg-tertiary-90 p-[10px]">
                                    <ThumbUpFill32
                                        width={20}
                                        height={20}
                                        className="text-tertiary-30"
                                    />
                                </div>
                            )}
                            <h3 className="text-[24px] text-headline-md text-neutral-10">
                                {isOnboardingFlow
                                    ? 'Wallet Created Successfully!'
                                    : 'Backup Recovery Phrase'}
                            </h3>
                        </div>
                        <div className="flex flex-col items-center gap-1 text-center">
                            <div className="text-[14px] text-title-sm text-neutral-60">
                                Mnemonic
                            </div>
                            <div className="text-[14px] text-title-sm text-neutral-60">
                                Your recovery phrase makes it easy to
                                <br />
                                back up and restore your account.
                            </div>
                        </div>

                        <div className="flex flex-grow flex-col flex-nowrap">
                            <Loading loading={passphraseMutation.isPending}>
                                {passphraseMutation.data ? (
                                    <>
                                        <TextArea
                                            value={passphraseMutation.data.join(' ')}
                                            isVisibilityToggleEnabled
                                        />
                                        {/*<HideShowDisplayBox*/}
                                        {/*    value={passphraseMutation.data}*/}
                                        {/*    hideCopy*/}
                                        {/*/>*/}
                                    </>
                                ) : (
                                    <Alert>
                                        {(passphraseMutation.error as Error)?.message ||
                                            'Something went wrong'}
                                    </Alert>
                                )}
                            </Loading>
                        </div>
                        <div>
                            <div className="mb-1 text-[14px] text-title-sm text-neutral-60">
                                Warning
                            </div>
                            <div className="text-[14px] text-title-sm text-neutral-60">
                                Never disclose your secret recovery phrase.
                                <br />
                                Anyone can take over your account with it.
                            </div>
                        </div>
                    </div>
                    <div className={'flex w-full flex-col gap-2'}>
                        {isOnboardingFlow ? (
                            <div className="flex w-full text-left">
                                <Checkbox
                                    label={'I saved my recovery phrase'}
                                    onChange={() => setPasswordCopied(!passwordCopied)}
                                />
                            </div>
                        ) : null}
                        <Button
                            onClick={() => navigate('/')}
                            type={ButtonType.Primary}
                            disabled={!passwordCopied && isOnboardingFlow}
                            text="Open Wallet"
                        />
                    </div>
                </div>
            )}
        </Loading>
    );
}
