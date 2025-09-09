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

export function ImportKeystone() {
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();

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

    function onError(_error: string) {}

    return (
        <PageTemplate title="Import Keystone" isTitleCentered showBackButton>
            <div className="flex h-full w-full flex-col items-center ">
                <div className="w-full grow">
                    <div className="flex h-full flex-col justify-between gap-2">
                        <div className="flex flex-col gap-sm">
                            <AnimatedQRScanner
                                handleScan={onSucceed}
                                handleError={onError}
                                urTypes={[URType.CryptoMultiAccounts]}
                            />
                        </div>
                        <div className="flex flex-row justify-stretch gap-2.5">
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
