// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Header, LoadingIndicator } from '@iota/apps-ui-kit';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from './layout/Layout';
import { ExplorerLink } from '../ExplorerLink';
import {
    ExplorerLinkType,
    TransactionReceipt,
    useGetTransactionWithSummary,
    ViewTxnOnExplorerButton,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { Validator } from './Staking/views/Validator';

interface SharedProps {
    txDigest?: string | null;
}

interface TransactionViewProps extends SharedProps {
    onClose: () => void;
    onBack?: () => void;
}

export function TransactionDialogView({
    txDigest,
    onClose,
    onBack,
}: TransactionViewProps): React.JSX.Element | null {
    const { data: transaction, summary } = useGetTransactionWithSummary(txDigest ?? '');
    const activeAddress = useCurrentAccount()?.address ?? '';

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
                        renderValidatorLogo={Validator}
                    />
                ) : (
                    <LoadingIndicator />
                )}
            </DialogLayoutBody>
            <DialogLayoutFooter>
                <ExplorerLink transactionID={txDigest ?? ''} type={ExplorerLinkType.Transaction}>
                    <ViewTxnOnExplorerButton digest={txDigest ?? ''} />
                </ExplorerLink>
            </DialogLayoutFooter>
        </DialogLayout>
    );
}
