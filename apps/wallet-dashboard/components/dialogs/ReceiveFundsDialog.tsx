// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Button,
    Address,
    Dialog,
    DialogContent,
    DialogBody,
    Header,
    Panel,
} from '@iota/apps-ui-kit';
import { QR, useGetDefaultIotaName, useCopyToClipboard } from '@iota/core';
import { trackElementCopied } from '@/lib/utils';
import { useCallback } from 'react';

interface ReceiveFundsDialogProps {
    address: string;
    setOpen: (bool: boolean) => void;
    open: boolean;
}

export function ReceiveFundsDialog({
    address,
    open,
    setOpen,
}: ReceiveFundsDialogProps): React.JSX.Element {
    const { data: iotaName } = useGetDefaultIotaName(address);
    const copyToClipboard = useCopyToClipboard(
        () => trackElementCopied('address'),
        'Address copied',
    );

    const handleCopyAddress = useCallback(() => {
        copyToClipboard(address);
    }, [copyToClipboard, address]);

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="Receive" onClose={() => setOpen(false)} />
                <DialogBody>
                    <div
                        className="flex max-h-[500px] flex-col gap-lg overflow-y-auto text-center [&_span]:w-full [&_span]:break-words"
                        data-amp-mask
                    >
                        <div className="self-center">
                            <QR value={address} size={130} marginSize={2} />
                        </div>

                        <div className="flex flex-col gap-xs">
                            {iotaName && (
                                <Panel bgColor="bg-iota-neutral-96 dark:bg-iota-neutral-12">
                                    <div className="break-words px-md--rs py-xs text-title-lg text-iota-neutral-12 dark:text-iota-neutral-96">
                                        {iotaName}
                                    </div>
                                </Panel>
                            )}

                            <Panel bgColor="bg-iota-neutral-96 dark:bg-iota-neutral-12">
                                <div className="px-md--rs py-xs text-title-lg text-iota-neutral-12 dark:text-iota-neutral-96">
                                    <Address text={address} />
                                </div>
                            </Panel>
                        </div>
                    </div>
                </DialogBody>
                <div className="flex w-full flex-row justify-center gap-2 px-md--rs pb-md--rs">
                    <Button onClick={handleCopyAddress} fullWidth text="Copy Address" />
                </div>
            </DialogContent>
        </Dialog>
    );
}
