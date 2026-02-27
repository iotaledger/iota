// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Button,
    ButtonType,
    Dialog,
    DialogBody,
    DialogContent,
    Header,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
} from '@iota/apps-ui-kit';
import { fromHex } from '@iota/bcs';
import { toast } from '@iota/core';
import { toSerializedSignature } from '@iota/iota-sdk/cryptography';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { AnimatedQRCode, AnimatedQRScanner } from '@keystonehq/animated-qr';
import { UR, URType, KeystoneIotaSDK } from '@keystonehq/keystone-sdk';
import { createContext, useContext, useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { KeystoneSigningCanceledByUserError } from './keystoneErrors';
import { useAppSelector, useCheckCameraPermissionStatus, useFullscreenGuard } from '_hooks';
import { ExtensionViewType } from '../../redux/slices/app/appType';
import { Warning } from '@iota/apps-ui-icons';

export enum SignatureType {
    Transaction = 'transaction',
    Message = 'message',
}

interface KeystoneContextValue {
    requestSignature: (ur: UR, signatureType?: SignatureType) => Promise<string>;
}

const KeystoneContext = createContext<KeystoneContextValue | undefined>(undefined);

interface KeystoneProviderProps {
    children: React.ReactNode;
}

interface Request {
    ur: UR;
    signatureType: SignatureType;
    reply: (signature: string) => void;
    cancel: () => void;
}

export function KeystoneProvider({ children }: KeystoneProviderProps) {
    const [currentRequest, setCurrentRequest] = useState<Request | null>(null);
    const [goFullscreen, setGoFullscreen] = useState(false);
    useFullscreenGuard(goFullscreen);

    const isFullscreen = useAppSelector(
        (state) => state.app.extensionViewType === ExtensionViewType.FullScreen,
    );

    useEffect(() => {
        if (currentRequest) {
            (async () => {
                try {
                    const permission = await navigator.permissions.query({
                        name: 'camera' as PermissionName,
                    });

                    if (permission.state === 'prompt') {
                        // Prompt won't show up in Popup mode, so we force fullscreen
                        setGoFullscreen(true);
                    }
                } catch (_) {
                    toast.error('Could not check camera permission status!');
                }
            })();
        }
    }, [currentRequest]);

    const context = useMemo(() => {
        return {
            requestSignature: (ur: UR, signatureType = SignatureType.Transaction) =>
                new Promise<string>((resolve, reject) => {
                    setCurrentRequest({
                        ur,
                        signatureType,
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
            {currentRequest && !(goFullscreen && !isFullscreen) ? (
                <ScanBothWays request={currentRequest} />
            ) : null}
        </KeystoneContext.Provider>
    );
}

enum Step {
    // Wallet renders and Keystone scans
    ShowQr,
    // Keystone renders  and Wallet scans
    ScanQr,
}

export function ScanBothWays({
    request: { ur, signatureType, reply, cancel },
}: {
    request: Request;
}) {
    const [step, setStep] = useState<Step>(Step.ShowQr);
    const [cameraPermissionStatus] = useCheckCameraPermissionStatus();

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

    const headerTitle =
        signatureType === SignatureType.Message ? 'Confirm Message' : 'Confirm Transaction';

    return (
        <Dialog open onOpenChange={(open) => {}}>
            <DialogContent containerId="overlay-portal-container">
                <Header title={headerTitle} titleCentered onClose={() => onCancel()} />
                <DialogBody>
                    <div className="flex flex-col items-center gap-2">
                        {step === Step.ShowQr ? (
                            <AnimatedQRCode
                                type={ur.type}
                                cbor={ur.cbor.toString('hex')}
                                options={{ size: 220 }}
                            />
                        ) : (
                            <div className="relative box-border flex h-[220px] w-[220px] items-center justify-center overflow-hidden rounded-lg">
                                <div className="flex-shrink-0">
                                    <AnimatedQRScanner
                                        key={cameraPermissionStatus}
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
                                {cameraPermissionStatus === 'prompt' ? (
                                    <div className="absolute inset-0 z-10 flex items-center justify-center">
                                        <InfoBox
                                            title="Camera Access authorization pending."
                                            supportingText={
                                                'Make sure your camera is connected and authorized, then try again to proceed.'
                                            }
                                            style={InfoBoxStyle.Elevated}
                                            type={InfoBoxType.Warning}
                                            icon={<Warning />}
                                        />
                                    </div>
                                ) : null}
                                {cameraPermissionStatus === 'denied' ? (
                                    <div className="absolute inset-0 z-10 flex items-center justify-center">
                                        <InfoBox
                                            title="Camera Access Blocked!"
                                            supportingText={
                                                'Please allow camera access, then try again to proceed.'
                                            }
                                            style={InfoBoxStyle.Elevated}
                                            type={InfoBoxType.Error}
                                            icon={<Warning />}
                                        />
                                    </div>
                                ) : null}
                            </div>
                        )}
                        <div className="flex flex-col items-center justify-center">
                            <Link
                                to="https://docs.iota.org/users/iota-wallet/how-to/basics#using-keystone-wallet"
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
                                    to="https://docs.iota.org/users/iota-wallet/how-to/basics#using-keystone-wallet"
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
