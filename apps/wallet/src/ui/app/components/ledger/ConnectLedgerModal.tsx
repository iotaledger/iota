// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { useIotaLedgerClient } from '_components';
import { useState } from 'react';
import { Button, ButtonType, Dialog, DialogBody, DialogContent, Header } from '@iota/apps-ui-kit';
import { Link } from 'react-router-dom';
import { ArrowTopRight } from '@iota/apps-ui-icons';

interface ConnectLedgerModalProps {
    onClose: () => void;
    onConfirm: () => void;
    onError: (error: unknown) => void;
}

export function ConnectLedgerModal({ onClose, onConfirm, onError }: ConnectLedgerModalProps) {
    const [, setConnectingToLedger] = useState(false);
    const { connectToLedger } = useIotaLedgerClient();

    const onContinueClick = async () => {
        try {
            setConnectingToLedger(true);
            await connectToLedger(true);
            onConfirm();
        } catch (error) {
            onError(error);
        } finally {
            setConnectingToLedger(false);
        }
    };

    return (
        <Dialog
            open
            onOpenChange={(open) => {
                if (!open) {
                    onClose();
                }
            }}
        >
            <DialogContent containerId="overlay-portal-container">
                <Header title="Connect Ledger Wallet" onClose={onClose} titleCentered />
                <DialogBody>
                    <div className="flex flex-col items-center gap-y-md">
                        <div className="flex w-full justify-center p-md">
                            <LedgerLogo />
                        </div>
                        <span className="text-center text-body-md text-neutral-40 dark:text-neutral-60">
                            Connect your ledger to your computer, unlock it, and launch the IOTA
                            app. Click Continue when done.
                        </span>
                        <div className="flex w-full flex-col gap-y-xs rounded-lg bg-primary-90 px-md py-sm text-center dark:bg-primary-10">
                            <span className="text-title-sm text-neutral-10 dark:text-neutral-92">
                                Ensure you're using a compatible version of the Ledger IOTA app
                            </span>
                            <div className="flex w-full flex-wrap justify-center gap-xs text-body-sm text-primary-30 dark:text-primary-80">
                                <Link
                                    to="https://docs.iota.org/about-iota/iota-wallet/how-to/integrate-ledger"
                                    target="_blank"
                                    rel="noreferrer"
                                    className="flex items-center gap-x-xxs underline"
                                >
                                    <span className="shrink-0">Recommended v0.9.3+</span>
                                    <ArrowTopRight />
                                </Link>
                                <Link
                                    to="https://docs.iota.org/about-iota/iota-wallet/how-to/integrate-ledger#manually-for-deprecated-devices-such-as-ledger-nano-s"
                                    target="_blank"
                                    rel="noreferrer"
                                    className="flex items-center gap-x-xxs underline"
                                >
                                    <span className="shrink-0">Older Devices v0.9.2+</span>
                                    <ArrowTopRight />
                                </Link>
                            </div>
                        </div>

                        <div className="flex items-center justify-center gap-x-1">
                            <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                                Need more help?
                            </span>
                            <Link
                                to="https://support.ledger.com/article/360011633353-zd"
                                onClick={() => ampli.viewedLedgerTutorial()}
                                className="text-body-md text-primary-30 no-underline dark:text-primary-80"
                                target="_blank"
                                rel="noreferrer"
                            >
                                View tutorial.
                            </Link>
                        </div>
                        <div className="flex w-full flex-row gap-x-xs">
                            <Button
                                type={ButtonType.Secondary}
                                text="Cancel"
                                onClick={onClose}
                                fullWidth
                            />
                            <Button
                                type={ButtonType.Primary}
                                text="Continue"
                                onClick={onContinueClick}
                                fullWidth
                            />
                        </div>
                    </div>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}

function LedgerLogo() {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="244"
            height="82"
            viewBox="0 0 244 82"
            fill="none"
            className="h-auto w-2/3 text-neutral-10 dark:text-neutral-92"
        >
            <path
                d="M208.725 76.3995V81.4987H244V58.5003H238.86V76.3995H208.725ZM208.725 0.5V5.59948H238.86V23.4997H244V0.5H208.725ZM190.534 39.9502V28.1006H198.597C202.528 28.1006 203.939 29.4003 203.939 32.9508V35.0504C203.939 38.7002 202.578 39.9502 198.597 39.9502H190.534ZM203.333 42.0498C207.012 41.0999 209.581 37.6994 209.581 33.6503C209.581 31.1005 208.574 28.8001 206.659 26.9498C204.24 24.6493 201.014 23.4997 196.833 23.4997H185.494V58.4991H190.534V44.5499H198.093C201.973 44.5499 203.536 46.1497 203.536 50.1504V58.5003H208.675V50.9503C208.675 45.4503 207.365 43.3507 203.333 42.7505V42.0498ZM160.904 43.1994H176.425V38.5997H160.904V28.0994H177.936V23.4997H155.763V58.4991H178.692V53.8994H160.904V43.1994ZM144.021 45.0497V47.4494C144.021 52.4993 142.157 54.1498 137.471 54.1498H136.363C131.675 54.1498 129.408 52.6493 129.408 45.6995V36.2993C129.408 29.2999 131.776 27.849 136.463 27.849H137.47C142.055 27.849 143.517 29.5491 143.567 34.2493H149.11C148.607 27.3492 143.971 23 137.016 23C133.64 23 130.818 24.0503 128.702 26.0495C125.527 28.9998 123.763 34 123.763 40.9994C123.763 47.7495 125.276 52.7497 128.399 55.8489C130.515 57.8989 133.439 58.9988 136.311 58.9988C139.335 58.9988 142.107 57.7984 143.517 55.1991H144.222V58.4991H148.858V40.45H135.201V45.0497H144.021ZM99.5765 28.0994H105.07C110.261 28.0994 113.083 29.3991 113.083 36.3997V45.5991C113.083 52.5985 110.261 53.8994 105.07 53.8994H99.5765V28.0994ZM105.522 58.5003C115.148 58.5003 118.725 51.2504 118.725 41.0006C118.725 30.6007 114.895 23.5009 105.421 23.5009H94.536V58.5003H105.522ZM70.1978 43.1994H85.7189V38.5997H70.1978V28.0994H87.23V23.4997H65.0566V58.4991H87.9862V53.8994H70.1978V43.1994ZM40.4664 23.4997H35.3268V58.4991H58.5074V53.8994H40.4664V23.4997ZM0.000976562 58.5003V81.5H35.2756V76.3995H5.14057V58.5003H0.000976562ZM0.000976562 0.5V23.4997H5.14057V5.59948H35.2756V0.5H0.000976562Z"
                fill="currentColor"
            />
        </svg>
    );
}
