// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Button,
    ButtonType,
    Checkbox,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    TextArea,
} from '@iota/apps-ui-kit';
import { Exclamation, Info } from '@iota/ui-icons';
import { useEffect, useMemo, useState } from 'react';
import toast from 'react-hot-toast';
import { Navigate, useNavigate, useParams } from 'react-router-dom';

import { PageTemplate } from '_components/PageTemplate';
import Loading from '_components/loading';
import { AccountSourceType } from '_src/background/account-sources/AccountSource';
import { useAccountSources } from '../../hooks/useAccountSources';
import { useExportPassphraseMutation } from '../../hooks/useExportPassphraseMutation';

export function BackupMnemonicPage() {
    const [mnemonicCopied, setMnemonicCopied] = useState(false);
    const [mnemonicBackedUp, setMnemonicBackedUp] = useState(false);

    const { accountSourceID } = useParams();
    const { data: accountSources, isPending } = useAccountSources();
    const selectedSource = useMemo(
        () => accountSources?.find(({ id }) => accountSourceID === id),
        [accountSources, accountSourceID],
    );
    const passphraseMutation = useExportPassphraseMutation();

    const navigate = useNavigate();
    if (!isPending && selectedSource?.type !== AccountSourceType.Mnemonic) {
        return <Navigate to="/" replace />;
    }

    useEffect(() => {
        (async () => {
            if (!passphraseMutation.isIdle || !accountSourceID) {
                return;
            }
            passphraseMutation.mutate({ accountSourceID: accountSourceID });
        })();
    }, [accountSourceID, passphraseMutation]);

    async function handleCopy() {
        if (!passphraseMutation?.data) {
            return;
        }
        try {
            await navigator.clipboard.writeText(passphraseMutation.data.join(' '));
            setMnemonicCopied(true);
            setTimeout(() => {
                setMnemonicCopied(false);
            }, 1000);
        } catch {
            toast.error('Failed to copy');
        }
    }

    return (
        <PageTemplate title="Export Mnemonic" isTitleCentered>
            <Loading loading={isPending}>
                <div className="flex h-full flex-col items-center justify-between">
                    <div className="flex flex-col gap-md">
                        <h3 className="text-center text-headline-lg text-neutral-10">
                            Wallet Created Successfully!
                        </h3>
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
                                    <InfoBox
                                        type={InfoBoxType.Default}
                                        supportingText={
                                            (passphraseMutation.error as Error)?.message ||
                                            'Something went wrong'
                                        }
                                        icon={<Exclamation />}
                                        style={InfoBoxStyle.Elevated}
                                    />
                                )}
                            </Loading>
                        </div>
                        {passphraseMutation.data && (
                            <div className="flex justify-end">
                                <Button
                                    onClick={handleCopy}
                                    type={ButtonType.Secondary}
                                    text={mnemonicCopied ? 'Copied' : 'Copy'}
                                />
                            </div>
                        )}
                    </div>
                    <div className="flex w-full flex-col">
                        <div className="flex w-full py-sm--rs">
                            <Checkbox
                                name="recovery-phrase"
                                label="I saved my mnemonic"
                                onCheckedChange={() => setMnemonicBackedUp(!mnemonicBackedUp)}
                            />
                        </div>
                        <div className="pt-sm--rs" />
                        <Button
                            onClick={() => navigate('/')}
                            type={ButtonType.Primary}
                            disabled={!mnemonicBackedUp}
                            text="Open Wallet"
                        />
                    </div>
                </div>
            </Loading>
        </PageTemplate>
    );
}
