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
import { QR, useCopyToClipboard, toast, useGetDefaultIotaName } from '@iota/core';

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
    const copyToClipboard = useCopyToClipboard();
    const { data: iotaName } = useGetDefaultIotaName(address);

    async function handleCopyToClipboard() {
        const success = await copyToClipboard(address);
        if (success) {
            toast('Address copied');
        }
    }

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="Receive" onClose={() => setOpen(false)} />
                <DialogBody>
                    <div className="flex flex-col gap-lg text-center [&_span]:w-full [&_span]:break-words">
                        <div className="self-center">
                            <QR value={address} size={130} marginSize={2} />
                        </div>

                        <div className="flex flex-col gap-xs">
                            {iotaName && (
                                <Panel bgColor="bg-iota-neutral-96 dark:bg-iota-neutral-12">
                                    <div className="px-md--rs py-xs text-title-lg text-iota-neutral-12 dark:text-iota-neutral-96">
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
                    <Button onClick={handleCopyToClipboard} fullWidth text="Copy Address" />
                </div>
            </DialogContent>
        </Dialog>
    );
}
