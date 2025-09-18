// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import classNames from 'clsx';
import { Link, useNavigate } from 'react-router-dom';
import { AccountsFormType, useAccountsFormContext, PageTemplate, AccountList } from '_components';
import { AnimatedQRScanner } from '@keystonehq/animated-qr';
import { Button, ButtonType, InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { UR, URType } from '@keystonehq/keystone-sdk';
import { parseMultiAccounts } from '@keystonehq/keystone-sdk/dist/wallet';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { fromHex, toBase64 } from '@iota/iota-sdk/utils';
import { toast } from '@iota/core';
import { useState } from 'react';
import { useAccounts, useCheckCameraPermissionStatus } from '../../hooks';
import { ImportPass, IotaLogoMark, QrCode, Warning } from '@iota/apps-ui-icons';

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
    const [scanProgress, setScanProgress] = useState(0);
    const [cameraPermissionStatus] = useCheckCameraPermissionStatus();

    function onSucceed({ type, cbor }: { type: string; cbor: string }) {
        const multiAccounts = parseMultiAccounts(new UR(Buffer.from(cbor, 'hex'), type));
        const iotaAccounts = multiAccounts.keys.filter((key) => key.chain === 'IOTA');
        const accounts = iotaAccounts.map((account) => ({
            publicKey: toBase64(fromHex(account.publicKey)),
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

    function onProgress(progress: number) {
        setScanProgress(progress);
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
        setScanProgress(0);
        toast.error(error);
    }

    const disableFinish = step.type === 'select-accounts' && step.selectedAccounts.size === 0;
    const canShowQrScanner = cameraPermissionStatus && cameraPermissionStatus !== 'denied';

    return (
        <PageTemplate title="Import Keystone" isTitleCentered showBackButton>
            <div className="flex h-full w-full flex-col items-center">
                <div className="w-full grow">
                    <div className="flex h-full flex-col justify-between gap-1">
                        {step.type === 'scan-qr' ? (
                            <>
                                <div
                                    className={classNames(
                                        'relative flex flex-col items-center justify-center gap-xs',
                                        { 'h-full justify-around': !canShowQrScanner },
                                    )}
                                >
                                    {canShowQrScanner ? (
                                        <>
                                            <div className="relative box-border flex h-[220px] w-[220px] items-center justify-center overflow-hidden rounded-lg">
                                                <div className="flex-shrink-0">
                                                    <AnimatedQRScanner
                                                        handleScan={onSucceed}
                                                        handleError={onError}
                                                        urTypes={[URType.CryptoMultiAccounts]}
                                                        options={{
                                                            blur: true,
                                                            width: '230px',
                                                            height: '230px',
                                                        }}
                                                        onProgress={onProgress}
                                                    />
                                                    {scanProgress > 0 && scanProgress <= 100 && (
                                                        <div className="absolute inset-0 flex items-end justify-center pb-2">
                                                            <div className="text-xl font-bold text-white">
                                                                {Math.round(scanProgress)}%
                                                            </div>
                                                        </div>
                                                    )}
                                                </div>
                                            </div>
                                            <span className="mb-sm text-center text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                                Camera is blurred for security reasons
                                            </span>
                                        </>
                                    ) : (
                                        <InfoBox
                                            title="Camera Access Blocked!"
                                            supportingText={
                                                'Please allow camera access, then try again to proceed.'
                                            }
                                            style={InfoBoxStyle.Elevated}
                                            type={InfoBoxType.Error}
                                            icon={<Warning />}
                                        />
                                    )}
                                    <div className="input-border-color flex w-full flex-col gap-xs rounded-2lg border border-solid p-4 no-underline">
                                        <div className="flex">
                                            <div className="mr-4 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-iota-primary-90 [&_svg]:h-4 [&_svg]:w-4 [&_svg]:text-black">
                                                <IotaLogoMark />
                                            </div>
                                            <div className="flex flex-col">
                                                <span className="text-iota-neutral-40 dark:text-iota-neutral-60">
                                                    Step 1
                                                </span>
                                                <span className="font-semibold text-iota-neutral-40 dark:text-iota-neutral-60">
                                                    Open the IOTA Wallet app in Keystone
                                                </span>
                                            </div>
                                        </div>
                                        <div className="flex">
                                            <div className="mr-4 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-iota-primary-90 [&_svg]:h-4 [&_svg]:w-4 [&_svg]:text-black">
                                                <QrCode />
                                            </div>
                                            <div className="flex flex-col">
                                                <span className="text-iota-neutral-40 dark:text-iota-neutral-60">
                                                    Step 2
                                                </span>
                                                <span className="font-semibold text-iota-neutral-40 dark:text-iota-neutral-60">
                                                    Point the QR code to your camera
                                                </span>
                                            </div>
                                        </div>
                                        <div className="flex">
                                            <div className="mr-4 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-iota-primary-90 [&_svg]:h-4 [&_svg]:w-4 [&_svg]:text-black">
                                                <ImportPass />
                                            </div>
                                            <div className="flex flex-col">
                                                <span className="text-iota-neutral-40 dark:text-iota-neutral-60">
                                                    Step 3
                                                </span>
                                                <span className="font-semibold text-iota-neutral-40 dark:text-iota-neutral-60">
                                                    Import wallets
                                                </span>
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                <div className="flex flex-col">
                                    <div className="mb-2 flex items-center justify-center gap-x-1">
                                        <span className="text-body-md text-iota-neutral-40 dark:text-iota-neutral-60">
                                            Need more help?
                                        </span>
                                        <Link
                                            // TODO: Add tutorial docs links when available - https://github.com/iotaledger/iota/issues/8511
                                            to=""
                                            className="text-body-md text-iota-primary-30 no-underline dark:text-iota-primary-80"
                                            target="_blank"
                                            rel="noreferrer"
                                        >
                                            View tutorial.
                                        </Link>
                                    </div>
                                    <div className="flex flex-row justify-stretch gap-2">
                                        <Button
                                            type={ButtonType.Secondary}
                                            text="Back"
                                            onClick={() => navigate(-1)}
                                            fullWidth
                                        />
                                    </div>
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
