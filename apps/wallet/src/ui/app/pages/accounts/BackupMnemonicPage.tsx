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
} from '@iota/apps-ui-kit';
import { Exclamation, Info } from '@iota/apps-ui-icons';
import { HideShowDisplayBox, Loading, PageTemplate } from '_components';
import { AccountSourceType } from '_src/background/account-sources/accountSource';
import { useEffect, useMemo, useState } from 'react';
import { Navigate, useNavigate, useParams } from 'react-router-dom';
import { useAccountSources, useExportPassphraseMutation } from '_hooks';

export function BackupMnemonicPage() {
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

    return (
        <PageTemplate title="Export Mnemonic" isTitleCentered>
            <Loading loading={isPending}>
                <div className="flex h-full flex-col items-center justify-between">
                    <div className="flex flex-col gap-md">
                        <h3 className="text-center text-headline-lg text-neutral-10 dark:text-neutral-92">
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
                                    <HideShowDisplayBox
                                        value={passphraseMutation.data.join(' ')}
                                        copiedMessage="Mnemonic copied"
                                    />
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
