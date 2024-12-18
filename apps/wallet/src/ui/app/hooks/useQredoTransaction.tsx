// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from 'react';
import { v4 as uuidV4 } from 'uuid';

import Alert from '../components/alert';
import LoadingIndicator from '../components/loading/LoadingIndicator';
import { QredoEvents, type QredoEventsType } from '../QredoSigner';
import { Button } from '../shared/ButtonUI';
import { Dialog, DialogContent } from '../shared/Dialog';
import { Heading } from '../shared/heading';
import { Text } from '../shared/text';

export function useQredoTransaction(preventModalDismiss?: boolean) {
    const [clientIdentifier, setClientIdentifier] = useState(() => uuidV4());
    const [qredoTransactionID, setQredoTransactionID] = useState<string | null>(null);
    const notificationModal = (
        <Dialog
            open={!!qredoTransactionID}
            onOpenChange={(open) => {
                if (!open && !preventModalDismiss) {
                    QredoEvents.emit('clientIgnoredUpdates', {
                        clientIdentifier,
                    });
                    setQredoTransactionID(null);
                }
            }}
        >
            <DialogContent>
                <div className="relative flex flex-col items-center gap-2.5 text-center">
                    {preventModalDismiss ? (
                        <Alert mode="warning">
                            Don't close this window until the transaction is completed
                        </Alert>
                    ) : null}
                    <div className="h-14 w-14 bg-[url('_assets/images/qredo.png')] bg-cover" />
                    <div className="text-steel">
                        <LoadingIndicator color="inherit" />
                    </div>
                    <Heading variant="heading6" color="gray-90" weight="medium">
                        Awaiting transaction approval in the Qredo app
                    </Heading>
                    <Text variant="pBodySmall" color="steel-dark">
                        Check your Qredo app for status. Once all required custody approvals have
                        been performed the transaction will complete.
                    </Text>
                    {!preventModalDismiss ? (
                        <Button
                            text="Close"
                            onClick={() => {
                                QredoEvents.emit('clientIgnoredUpdates', {
                                    clientIdentifier,
                                });
                                setQredoTransactionID(null);
                            }}
                        />
                    ) : null}
                </div>
            </DialogContent>
        </Dialog>
    );
    useEffect(() => {
        function qredoTransactionCreated(event: QredoEventsType['qredoTransactionCreated']) {
            if (event.clientIdentifier === clientIdentifier) {
                setQredoTransactionID(event.qredoTransaction.txID);
            }
        }
        function qredoTransactionComplete(event: QredoEventsType['qredoActionDone']) {
            if (event.clientIdentifier === clientIdentifier) {
                setQredoTransactionID(null);
                setClientIdentifier(uuidV4());
            }
        }
        QredoEvents.on('qredoTransactionCreated', qredoTransactionCreated);
        QredoEvents.on('qredoActionDone', qredoTransactionComplete);
        return () => {
            QredoEvents.off('qredoTransactionCreated', qredoTransactionCreated);
            QredoEvents.off('qredoActionDone', qredoTransactionComplete);
            QredoEvents.emit('clientIgnoredUpdates', { clientIdentifier });
        };
    }, [clientIdentifier]);
    return {
        clientIdentifier,
        notificationModal,
    };
}
