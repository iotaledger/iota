// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from 'react';
import {
    Button,
    Address,
    Dialog,
    DialogContent,
    DialogBody,
    Header,
    Panel,
} from '@iota/apps-ui-kit';
import { useCopyToClipboard, useActiveAccount } from '_hooks';
import { QR, toast, useGetDefaultIotaName } from '@iota/core';
import { useIotaLedgerClient } from '_src/ui/app/components';
import {
    isLedgerAccountSerializedUI,
    type LedgerAccountSerializedUI,
} from '_src/background/accounts/ledgerAccount';

interface ReceiveTokensDialogProps {
    address: string;
    open: boolean;
    setOpen: (isOpen: boolean) => void;
}

export function ReceiveTokensDialog({ address, open, setOpen }: ReceiveTokensDialogProps) {
    const activeAccount = useActiveAccount();
    const { connectToLedger, iotaLedgerClient } = useIotaLedgerClient();
    const { data: iotaName } = useGetDefaultIotaName(address);

    const onCopy = useCopyToClipboard(address, {
        copySuccessMessage: 'Address copied',
        textType: 'address',
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
                ledgerClient = await connectToLedger();
            }

            toast('Please, confirm the address on your Ledger device.');
            await ledgerClient.getPublicKey(activeAccount.derivationPath, true);
            toast.success('Address verification successful!');
        } catch {
            toast.error('Address verification failed!');
        }
    }, [isLedger, activeAccount, iotaLedgerClient, connectToLedger]);

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
                                <div
                                    className="px-md--rs py-xs text-title-lg text-iota-neutral-12 dark:text-iota-neutral-96"
                                    data-testid="receive-address"
                                >
                                    <Address text={address} />
                                </div>
                            </Panel>
                        </div>
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
    );
}
