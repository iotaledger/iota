// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonType, Dialog, DialogBody, DialogContent, Header } from '@iota/apps-ui-kit';
import { fromHex } from '@iota/bcs';
import { toast } from '@iota/core';
import { toSerializedSignature } from '@iota/iota-sdk/cryptography';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { AnimatedQRCode, AnimatedQRScanner } from '@keystonehq/animated-qr';
import { UR, URType, KeystoneIotaSDK } from '@keystonehq/keystone-sdk';
import { createContext, useContext, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { KeystoneSigningCanceledByUserError } from './keystoneErrors';

interface KeystoneContextValue {
    requestSignature: (ur: UR) => Promise<string>;
}

const KeystoneContext = createContext<KeystoneContextValue | undefined>(undefined);

interface KeystoneProviderProps {
    children: React.ReactNode;
}

interface Request {
    ur: UR;
    reply: (signature: string) => void;
    cancel: () => void;
}

export function KeystoneProvider({ children }: KeystoneProviderProps) {
    const [currentRequest, setCurrentRequest] = useState<Request | null>(null);

    const context = useMemo(() => {
        return {
            requestSignature: (ur: UR) =>
                new Promise<string>((resolve, reject) => {
                    setCurrentRequest({
                        ur,
                        reply: (signature) => {
                            setCurrentRequest(null);
                            resolve(signature);
                        },
                        cancel: () => {
                            reject(new KeystoneSigningCanceledByUserError('User canceled'));
                            setCurrentRequest(null);
                        },
                    });
                }),
        };
    }, []);

    return (
        <KeystoneContext.Provider value={context}>
            {children}
            {currentRequest ? <ScanBothWays request={currentRequest} /> : null}
        </KeystoneContext.Provider>
    );
}

enum Step {
    // Wallet renders and Keystone scans
    ShowQr,
    // Keystone renders  and Wallet scans
    ScanQr,
}

export function ScanBothWays({ request: { ur, reply, cancel } }: { request: Request }) {
    const [step, setStep] = useState<Step>(Step.ShowQr);

    function onSucceed({ type, cbor }: { type: string; cbor: string }) {
        const { signature, publicKey } = new KeystoneIotaSDK().parseSignature(
            new UR(Buffer.from(cbor, 'hex'), type),
        );
        reply(
            toSerializedSignature({
                signature: fromHex(signature),
                publicKey: new Ed25519PublicKey(fromHex(publicKey)),
                signatureScheme: 'ED25519',
            }),
        );
    }

    function onCancel() {
        cancel();
    }

    function onError(error: string) {
        toast.error(`Error while scanning QR: ${error}`);
    }

    return (
        <Dialog open onOpenChange={(open) => {}}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="Confirm Transaction" titleCentered onClose={() => onCancel()} />
                <DialogBody>
                    <div className="flex flex-col items-center gap-2">
                        {step === Step.ShowQr ? (
                            <AnimatedQRCode
                                type={ur.type}
                                cbor={ur.cbor.toString('hex')}
                                options={{ size: 220 }}
                            />
                        ) : (
                            <div className="box-border flex h-[220px] w-[220px] items-center justify-center overflow-hidden rounded-lg">
                                <div className="flex-shrink-0">
                                    <AnimatedQRScanner
                                        handleScan={onSucceed}
                                        handleError={onError}
                                        urTypes={[URType.IotaSignature]}
                                        options={{
                                            blur: true,
                                            width: '230px',
                                            height: '230px',
                                        }}
                                    />
                                </div>
                            </div>
                        )}
                        <div className="flex flex-col items-center justify-center">
                            <Link
                                // TODO: Add step 1/2 from tutorial docs link when available - https://github.com/iotaledger/iota/issues/8511
                                to=""
                                className="mb-1 text-body-md text-iota-primary-30 no-underline dark:text-iota-primary-80"
                                target="_blank"
                                rel="noreferrer"
                            >
                                {step === Step.ShowQr ? 'Step 1' : 'Step 2'}
                            </Link>
                            <span className="text-center text-body-md text-iota-neutral-40 dark:text-iota-neutral-60">
                                {step === Step.ShowQr
                                    ? 'Scan this QR code with your Keystone device, then press continue'
                                    : 'Scan the QR code displayed on your keystone device'}
                            </span>
                        </div>
                        <div className="flex w-full flex-col">
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
                            <div className="flex w-full gap-xs">
                                <Button
                                    fullWidth
                                    type={ButtonType.Secondary}
                                    text="Cancel"
                                    onClick={() => onCancel()}
                                />
                                {step === Step.ShowQr && (
                                    <Button
                                        fullWidth
                                        type={ButtonType.Primary}
                                        text="Continue"
                                        onClick={() => setStep(Step.ScanQr)}
                                    />
                                )}
                            </div>
                        </div>
                    </div>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}

export function useKeystoneContext() {
    const keystoneContext = useContext(KeystoneContext);
    if (!keystoneContext) {
        throw new Error('useKeystoneContext must be used within KeystoneProvider');
    }
    return keystoneContext;
}
