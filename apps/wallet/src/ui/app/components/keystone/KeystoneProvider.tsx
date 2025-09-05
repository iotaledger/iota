import { Button, Dialog, DialogBody, DialogContent, Header } from '@iota/apps-ui-kit';
import { fromHex } from '@iota/bcs';
import { toSerializedSignature } from '@iota/iota-sdk/cryptography';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { AnimatedQRCode, AnimatedQRScanner } from '@keystonehq/animated-qr';
import { UR, URType } from '@keystonehq/keystone-sdk';
import { KeystoneIotaSDK } from '@keystonehq/keystone-sdk';
import { createContext, useContext, useMemo, useState } from 'react';

interface KeystoneContextValue {
    requestSignature: (ur: UR) => Promise<string>;
}

const KeystoneContext = createContext<KeystoneContextValue | undefined>(undefined);

export function useKeystoneContext() {
    const keystoneContext = useContext(KeystoneContext);
    if (!keystoneContext) {
        throw new Error('useIotaLedgerClient must be used within IotaLedgerClientContext');
    }
    return keystoneContext;
}

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
    ShowQr,
    ScanQr,
}

export function ScanBothWays({ request: { ur, reply: ack } }: { request: Request }) {
    const [step, setState] = useState<Step>(Step.ShowQr);
    function onSucceed({ type, cbor }: { type: string; cbor: string }) {
        const { signature, publicKey } = new KeystoneIotaSDK().parseSignature(
            new UR(Buffer.from(cbor, 'hex'), type),
        );
        ack(
            toSerializedSignature({
                signature: fromHex(signature),
                publicKey: new Ed25519PublicKey(fromHex(publicKey)),
                signatureScheme: 'ED25519',
            }),
        );
    }

    function onError() {}

    return (
        <Dialog open onOpenChange={(open) => {}}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="Scan" titleCentered />
                <DialogBody>
                    <p>hi</p>
                    {step === Step.ShowQr ? (
                        <>
                            <AnimatedQRCode type={ur.type} cbor={ur.cbor.toString('hex')} />
                            <Button
                                text="Get Type Signature"
                                onClick={() => setState(Step.ScanQr)}
                            />
                        </>
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
