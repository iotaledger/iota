// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';
import { AccountsFormType, useAccountsFormContext, PageTemplate, AccountList } from '_components';
import { AnimatedQRScanner } from '@keystonehq/animated-qr';
import { Button, ButtonType, InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { UR, URType } from '@keystonehq/keystone-sdk';
import { parseMultiAccounts } from '@keystonehq/keystone-sdk/dist/wallet';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { fromHex } from '@iota/iota-sdk/utils';
import { toast } from '@iota/core';
import { useState } from 'react';
import { useAccounts } from '../../hooks';
import { Warning } from '@iota/apps-ui-icons';

type Step =
    | {
          // Wallet scans Keystone animated QR
          type: 'scan-qr';
      }
    | {
          // User selects from the account list
          type: 'select-accounts';
          masterFingerprint: string;
          accounts: {
              publicKey: string;
              derivationPath: string;
              address: string;
          }[];
          selectedAccounts: Set<string>;
      };

export function ImportKeystone() {
    const [step, setStep] = useState<Step>({ type: 'scan-qr' });
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();

    function onSucceed({ type, cbor }: { type: string; cbor: string }) {
        const multiAccounts = parseMultiAccounts(new UR(Buffer.from(cbor, 'hex'), type));
        const iotaAccounts = multiAccounts.keys.filter((key) => key.chain === 'IOTA');
        const accounts = iotaAccounts.map((account) => ({
            publicKey: account.publicKey,
            derivationPath: account.path,
            address: new Ed25519PublicKey(fromHex(account.publicKey)).toIotaAddress(),
        }));
        setStep({
            type: 'select-accounts',
            accounts,
            selectedAccounts: new Set(),
            masterFingerprint: multiAccounts.masterFingerprint,
        });
    }

    function onFinish() {
        if (step.type === 'select-accounts') {
            setAccountsFormValues({
                type: AccountsFormType.ImportKeystone,
                accounts: step.accounts.filter((account) =>
                    step.selectedAccounts.has(account.address),
                ),
                masterFingerprint: step.masterFingerprint,
            });
            navigate(
                `/accounts/protect-account?${new URLSearchParams({
                    accountsFormType: AccountsFormType.ImportKeystone,
                }).toString()}`,
            );
        }
    }

    function onError(error: string) {
        toast.error(error);
    }

    const disableFinish = step.type === 'select-accounts' && step.selectedAccounts.size === 0;

    return (
        <PageTemplate title="Import Keystone">
            <div className="flex h-full w-full flex-col items-center ">
                <div className="w-full grow">
                    <div className="flex h-full flex-col justify-between gap-2 ">
                        {step.type === 'scan-qr' ? (
                            <>
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
                            </>
                        ) : (
                            <>
                                <div className="max-h-[530px] w-full flex-col gap-y-sm overflow-auto overflow-y-auto">
                                    <KeystoneAccountsList step={step} setStep={setStep} />
                                </div>
                                <div className="flex flex-1 flex-row items-end justify-stretch gap-2.5">
                                    <Button
                                        type={ButtonType.Secondary}
                                        text="Go back"
                                        onClick={() => setStep({ type: 'scan-qr' })}
                                        fullWidth
                                    />
                                    <Button
                                        type={ButtonType.Primary}
                                        text="Finish"
                                        onClick={onFinish}
                                        fullWidth
                                        disabled={disableFinish}
                                    />
                                </div>
                            </>
                        )}
                    </div>
                </div>
            </div>
        </PageTemplate>
    );
}

function KeystoneAccountsList<S extends Extract<Step, { type: 'select-accounts' }>>({
    step,
    setStep,
}: {
    step: S;
    setStep: (step: S) => void;
}) {
    const { data: existingAccounts } = useAccounts();

    const eligibleAccounts = step.accounts.filter(
        (account) => !existingAccounts?.some((existing) => existing.address === account.address),
    );

    if (eligibleAccounts.length === 0) {
        return (
            <InfoBox
                icon={<Warning />}
                type={InfoBoxType.Warning}
                title={'All scanned accounts have already been imported.'}
                style={InfoBoxStyle.Default}
            />
        );
    }

    function onAccountClick(account: {
        publicKey: string;
        derivationPath: string;
        address: string;
    }) {
        if (step.selectedAccounts.has(account.address)) {
            step.selectedAccounts.delete(account.address);
        } else {
            step.selectedAccounts.add(account.address);
        }
        setStep({
            ...step,
            selectedAccounts: new Set(step.selectedAccounts),
        });
    }

    function onSelectAll() {
        const areAllAccountsSelected = step.selectedAccounts.size === eligibleAccounts.length;
        if (!areAllAccountsSelected) {
            const selectedAccounts = new Set(eligibleAccounts.map((acc) => acc.address));
            setStep({ ...step, selectedAccounts: selectedAccounts });
        } else if (areAllAccountsSelected) {
            setStep({ ...step, selectedAccounts: new Set() });
        }
    }

    return (
        <AccountList
            accounts={eligibleAccounts}
            onAccountClick={onAccountClick}
            selectedAccounts={step.selectedAccounts}
            selectAll={onSelectAll}
        />
    );
}
