// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from 'react';
import { Button, Address, Dialog, DialogContent, DialogBody, Header } from '@iota/apps-ui-kit';
import { useCopyToClipboard } from '_hooks';
import { QR } from '@iota/core';
import { toast } from 'react-hot-toast';
import { useIotaLedgerClient } from '_src/ui/app/components';
import {
    isLedgerAccountSerializedUI,
    type LedgerAccountSerializedUI,
} from '_src/background/accounts/ledgerAccount';
import { useActiveAccount } from '_src/ui/app/hooks/useActiveAccount';

interface ReceiveTokensDialogProps {
    address: string;
    open: boolean;
    setOpen: (isOpen: boolean) => void;
}

export function ReceiveTokensDialog({ address, open, setOpen }: ReceiveTokensDialogProps) {
    const activeAccount = useActiveAccount();
    const { connectToLedger, iotaLedgerClient } = useIotaLedgerClient();

    const onCopy = useCopyToClipboard(address, {
        copySuccessMessage: 'Address copied',
    });

    const isLedger = isLedgerAccountSerializedUI(activeAccount as LedgerAccountSerializedUI);

    const onVerifyAddress = useCallback(async () => {
        if (!isLedger || !activeAccount) {
            return;
        }

        if (!isLedgerAccountSerializedUI(activeAccount)) {
            return;
        }

        try {
            let ledgerClient = iotaLedgerClient;
            if (!ledgerClient) {
                ledgerClient = await connectToLedger(true);
            }

            toast.success('Please, confirm the address on your Ledger device.');
            await ledgerClient.getPublicKey(activeAccount.derivationPath, true);
            toast.success('Address verification successful!');
        } catch {
            toast.error('Address verification failed!');
        }
    }, [isLedger, activeAccount, iotaLedgerClient, connectToLedger]);

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
                    {isLedger && (
                        <div className="flex w-full flex-row justify-center gap-2 px-md--rs pb-md--rs pt-sm--rs">
                            <Button onClick={onVerifyAddress} fullWidth text="Verify Address" />
                        </div>
                    )}
                </DialogContent>
            </Dialog>
        </div>
    );
}
