// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, Address, Dialog, DialogContent, DialogBody, Header } from '@iota/apps-ui-kit';
import { useCopyToClipboard } from '_hooks';
import { QR } from '_src/ui/app/components';

interface ReceiveTokensDialogProps {
    address: string;
    open: boolean;
    setOpen: (isOpen: boolean) => void;
}

export function ReceiveTokensDialog({ address, open, setOpen }: ReceiveTokensDialogProps) {
    const onCopy = useCopyToClipboard(address, {
        copySuccessMessage: 'Address copied',
    });

    return (
        <div className="relative">
            <Dialog open={open} onOpenChange={setOpen}>
                <DialogContent containerId="overlay-portal-container">
                    <Header title="Receive" onClose={() => setOpen(false)} />
                    <DialogBody>
                        <div className="flex flex-col gap-lg text-center [&_span]:w-full [&_span]:break-words">
                            <div className="self-center">
                                <QR value={address} size={130} />
                            </div>
                            <Address text={address} />
                        </div>
                    </DialogBody>
                    <div className="flex w-full flex-row justify-center gap-2 px-md--rs pb-md--rs pt-sm--rs">
                        <Button onClick={onCopy} fullWidth text="Copy Address" />
                    </div>
                </DialogContent>
            </Dialog>
        </div>
    );
}
