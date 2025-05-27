// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Header, LoadingIndicator } from '@iota/apps-ui-kit';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from './layout';
import { ExplorerLink } from '../ExplorerLink';
import {
    ExplorerLinkType,
    OutlinedCopyButton,
    toast,
    TransactionReceipt,
    useGetTransactionWithSummary,
    ViewTxnOnExplorerButton,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';

interface TransactionViewProps {
    onClose: () => void;
    onBack?: () => void;
    txDigest: string | null;
}

export function TransactionDialogView({
    txDigest,
    onClose,
    onBack,
}: TransactionViewProps): React.JSX.Element | null {
    const activeAddress = useCurrentAccount()?.address ?? '';
    const { data: transaction, summary } = useGetTransactionWithSummary(
        txDigest ?? '',
        activeAddress,
    );

    return (
        <DialogLayout>
            <Header title="Transaction" onClose={onClose} onBack={onBack} titleCentered />
            <DialogLayoutBody>
                {transaction && summary ? (
                    <TransactionReceipt
                        txn={transaction}
                        activeAddress={activeAddress}
                        summary={summary}
                        renderExplorerLink={ExplorerLink}
                    />
                ) : (
                    <div className="flex h-full w-full justify-center">
                        <LoadingIndicator />
                    </div>
                )}
            </DialogLayoutBody>
            <DialogLayoutFooter>
                <div className="flex w-full flex-row gap-x-xs">
                    <div className="flex w-full [&_a]:w-full">
                        <ExplorerLink
                            transactionID={txDigest ?? ''}
                            type={ExplorerLinkType.Transaction}
                        >
                            <ViewTxnOnExplorerButton digest={txDigest ?? ''} />
                        </ExplorerLink>
                    </div>
                    <div className="self-center">
                        <OutlinedCopyButton
                            textToCopy={txDigest ?? ''}
                            onCopySuccess={() =>
                                toast.success('Transaction digest copied to clipboard')
                            }
                        />
                    </div>
                </div>
            </DialogLayoutFooter>
        </DialogLayout>
    );
}
