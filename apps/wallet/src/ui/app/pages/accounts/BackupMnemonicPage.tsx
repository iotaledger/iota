// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CardLayout } from '_app/shared/card-layout';
import Alert from '_components/alert';
import Loading from '_components/loading';
import PageTemplate from '_components/PageTemplate';
import {
    Button,
    ButtonType,
    Checkbox,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    TextArea,
} from '@iota/apps-ui-kit';
import { Info } from '@iota/ui-icons';
import { useEffect, useMemo, useState } from 'react';
import { Navigate, useLocation, useNavigate, useParams } from 'react-router-dom';
import toast from 'react-hot-toast';

import { VerifyPasswordModal } from '../../components/accounts/VerifyPasswordModal';
import { useAccountSources } from '../../hooks/useAccountSources';
import { useExportPassphraseMutation } from '../../hooks/useExportPassphraseMutation';
import { AccountSourceType } from '_src/background/account-sources/AccountSource';

export function BackupMnemonicPage() {
    const [passwordCopied, setPasswordCopied] = useState(false);
    const [passphraseCopied, setPassphraseCopied] = useState(false);

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

    const handleCopy = async () => {
        if (!passphraseMutation?.data) {
            return;
        }
        try {
            await navigator.clipboard.writeText(passphraseMutation.data.join(' '));
            setPassphraseCopied(true);
            setTimeout(() => {
                setPassphraseCopied(false);
            }, 1000);
        } catch {
            toast.error('Failed to copy');
        }
    };

    return (
        <PageTemplate title={'Export Mnemonic'} isTitleCentered>
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
                    <div
                        className={`flex h-full flex-col items-center ${isOnboardingFlow ? 'justify-between' : ''}`}
                    >
                        <div className="flex flex-col gap-md">
                            {isOnboardingFlow && (
                                <div className="flex flex-col items-center gap-md px-md py-sm text-center">
                                    <h3 className="text-headline-lg text-neutral-10">
                                        Wallet Created Successfully!
                                    </h3>
                                </div>
                            )}
                            <InfoBox
                                icon={<Info />}
                                type={InfoBoxType.Default}
                                title={
                                    'Never disclose your secret mnemonic. Anyone can take over your wallet with it.'
                                }
                                style={InfoBoxStyle.Default}
                            />

                            <div className="flex flex-grow flex-col flex-nowrap">
                                <Loading loading={passphraseMutation.isPending}>
                                    {passphraseMutation.data ? (
                                        <>
                                            <TextArea
                                                value={passphraseMutation.data.join(' ')}
                                                isVisibilityToggleEnabled
                                                rows={5}
                                            />
                                        </>
                                    ) : (
                                        <Alert>
                                            {(passphraseMutation.error as Error)?.message ||
                                                'Something went wrong'}
                                        </Alert>
                                    )}
                                </Loading>
                            </div>
                        </div>
                        {isOnboardingFlow ? (
                            <div className={'flex w-full flex-col'}>
                                <div className="flex w-full py-sm--rs">
                                    <Checkbox
                                        label="I saved my recovery phrase"
                                        onChange={() => setPasswordCopied(!passwordCopied)}
                                    />
                                </div>
                                <div className="pt-sm--rs" />
                                <Button
                                    onClick={() => navigate('/')}
                                    type={ButtonType.Primary}
                                    disabled={!passwordCopied && isOnboardingFlow}
                                    text="Open Wallet"
                                />
                            </div>
                        ) : (
                            <div className={'flex w-full flex-col pt-sm'}>
                                <Button
                                    onClick={handleCopy}
                                    type={ButtonType.Primary}
                                    text={passphraseCopied ? 'Copied' : 'Copy'}
                                />
                            </div>
                        )}
                    </div>
                )}
            </Loading>
        </PageTemplate>
    );
}
