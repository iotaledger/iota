// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, Address, Dialog, DialogContent, DialogBody, Header } from '@iota/apps-ui-kit';
import { QR, useCopyToClipboard, toast } from '@iota/core';

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
                        <Address text={address} />
                    </div>
                </DialogBody>
                <div className="flex w-full flex-row justify-center gap-2 px-md--rs pb-md--rs pt-sm--rs">
                    <Button onClick={handleCopyToClipboard} fullWidth text="Copy Address" />
                </div>
            </DialogContent>
        </Dialog>
    );
}
