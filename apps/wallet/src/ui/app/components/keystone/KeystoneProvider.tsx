// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, Dialog, DialogBody, DialogContent, Header } from '@iota/apps-ui-kit';
import { fromHex } from '@iota/bcs';
import { toast } from '@iota/core';
import { toSerializedSignature } from '@iota/iota-sdk/cryptography';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { AnimatedQRCode, AnimatedQRScanner } from '@keystonehq/animated-qr';
import { UR, URType, KeystoneIotaSDK } from '@keystonehq/keystone-sdk';
import { createContext, useContext, useMemo, useState } from 'react';

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
}

export function KeystoneProvider({ children }: KeystoneProviderProps) {
    const [currentRequest, setCurrentRequest] = useState<Request | null>(null);

    const context = useMemo(() => {
        return {
            requestSignature: (ur: UR) =>
                new Promise<string>((resolve) => {
                    setCurrentRequest({
                        ur,
                        reply: (signature) => {
                            setCurrentRequest(null);
                            resolve(signature);
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

export function ScanBothWays({ request: { ur, reply } }: { request: Request }) {
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

    function onError(error: string) {
        toast.error(`Error while scanning QR: ${error}`);
    }

    return (
        <Dialog open onOpenChange={(open) => {}}>
            <DialogContent containerId="overlay-portal-container">
                <Header
                    title={step === Step.ShowQr ? 'Scan with your Keystone' : 'Scan your keystone'}
                    titleCentered
                />
                <DialogBody>
                    {step === Step.ShowQr ? (
                        <div className="flex flex-col items-center gap-4">
                            <AnimatedQRCode
                                type={ur.type}
                                cbor={ur.cbor.toString('hex')}
                                options={{ size: 240 }}
                            />
                            <Button text="Get Signature" onClick={() => setStep(Step.ScanQr)} />
                        </div>
                    ) : (
                        <>
                            <AnimatedQRScanner
                                handleScan={onSucceed}
                                handleError={onError}
                                urTypes={[URType.IotaSignature]}
                            />
                        </>
                    )}
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
