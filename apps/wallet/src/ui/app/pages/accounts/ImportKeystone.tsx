// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';
import { AccountsFormType, useAccountsFormContext, PageTemplate } from '_components';
import { AnimatedQRScanner } from '@keystonehq/animated-qr';
import { Button, ButtonType } from '@iota/apps-ui-kit';
import { UR, URType } from '@keystonehq/keystone-sdk';
import { parseMultiAccounts } from '@keystonehq/keystone-sdk/dist/wallet';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { fromHex } from '@iota/iota-sdk/utils';
import { useState } from 'react';

export function ImportKeystone() {
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();
    const [scanProgress, setScanProgress] = useState(0);

    function onSucceed({ type, cbor }: { type: string; cbor: string }) {
        const multiAccounts = parseMultiAccounts(new UR(Buffer.from(cbor, 'hex'), type));
        const iotaAccounts = multiAccounts.keys.filter((key) => key.chain === 'IOTA');
        const accounts = iotaAccounts.map((account) => ({
            publicKey: account.publicKey,
            derivationPath: account.path,
            address: new Ed25519PublicKey(fromHex(account.publicKey)).toIotaAddress(),
            masterFingerprint: multiAccounts.masterFingerprint,
        }));
        setAccountsFormValues({
            type: AccountsFormType.ImportKeystone,
            accounts,
        });
        navigate(
            `/accounts/protect-account?${new URLSearchParams({
                accountsFormType: AccountsFormType.ImportKeystone,
            }).toString()}`,
        );
    }

    function onError(_error: string) {
        setScanProgress(0);
    }

    function onProgress(progress: number) {
        setScanProgress(progress);
    }

    return (
        <PageTemplate title="Import Keystone" isTitleCentered showBackButton>
            <div className="flex h-full w-full flex-col items-center">
                <div className="w-full grow">
                    <div className="flex h-full flex-col gap-2">
                        <div className="flex flex-col gap-sm">
                            <AnimatedQRScanner
                                handleScan={onSucceed}
                                handleError={onError}
                                urTypes={[URType.CryptoMultiAccounts]}
                                onProgress={onProgress}
                            />
                        </div>
                        {scanProgress > 0 && scanProgress <= 100 && (
                            <div className="mt-4 flex flex-row items-start gap-4 rounded-lg bg-default-surface py-xs pl-xs pr-lg">
                                <div className="flex w-full flex-col gap-1">
                                    <span className="infobox-text-title text-center text-title-sm">
                                        Scanning QR Code
                                    </span>
                                    <span className="infobox-supporting-text text-body-sm">
                                        <div className="flex w-full flex-col gap-2">
                                            <div className="text-center text-sm">
                                                Progress: {Math.round(scanProgress)}%
                                            </div>
                                            <div className="h-2 w-full overflow-hidden rounded-full bg-gray-200 dark:bg-gray-700">
                                                <div
                                                    className="h-full rounded-full bg-blue-600 transition-all duration-300 ease-out dark:bg-blue-500"
                                                    style={{ width: `${scanProgress}%` }}
                                                />
                                            </div>
                                        </div>
                                    </span>
                                </div>
                            </div>
                        )}
                        <div className="mt-auto flex flex-row justify-stretch gap-2.5">
                            <Button
                                type={ButtonType.Secondary}
                                text="Cancel"
                                onClick={() => navigate(-1)}
                                fullWidth
                            />
                        </div>
                    </div>
                </div>
            </div>
        </PageTemplate>
    );
}
