// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';
import {
    Dialog,
    DialogContent,
    DialogBody,
    Header,
    Button,
    ButtonType,
    LoadingIndicator,
    InfoBox,
    InfoBoxType,
    InfoBoxStyle,
} from '@iota/apps-ui-kit';
import { useActiveAddress, useTransactionData, useTransactionDryRun } from '_hooks';
import { ExplorerLinkHelper } from '_components';
import {
    TransactionSummary,
    GasFees,
    useTransactionSummary,
    useRecognizedPackages,
    useGetAllOwnedObjects,
    TIMELOCK_IOTA_TYPE,
    useGetTimelockedStakedObjects,
    createCollectAllTimelocksTransaction,
    mapTimelockObjects,
    DRY_RUN_UI_ERROR_TITLE,
    getUserFriendlyDryRunExecutionError,
    toast,
} from '@iota/core';
import type { Transaction } from '@iota/iota-sdk/transactions';
import { Warning } from '@iota/apps-ui-icons';
import { useSignAndExecuteTransaction } from '@iota/dapp-kit';

interface CollectAllTimelocksDialogProps {
    open: boolean;
    setOpen: (isOpen: boolean) => void;
    onSuccess?: () => void;
}

export function CollectAllTimelocksDialog({
    open,
    setOpen,
    onSuccess,
}: CollectAllTimelocksDialogProps) {
    const activeAddress = useActiveAddress();

    // Fetch timelock objects and timelock staked objects
    const { data: timelockObjects, isPending: isTimelocksLoading } = useGetAllOwnedObjects(
        activeAddress || '',
        {
            StructType: TIMELOCK_IOTA_TYPE,
        },
    );

    const { data: timelockedStakes, isPending: isTimelockedStakesLoading } =
        useGetTimelockedStakedObjects(activeAddress || '');

    const recognizedPackagesList = useRecognizedPackages();
    const { mutateAsync: signAndExecuteTransaction, isPending: isExecuting } =
        useSignAndExecuteTransaction();

    // Build the transaction
    const transaction = useMemo(() => {
        if (!activeAddress || !timelockObjects || !timelockedStakes) return null;

        const mappedTimelocks = mapTimelockObjects(timelockObjects);
        const timelockObjectIds = mappedTimelocks.map((tl) => tl.id.id);

        // Get all timelocked staked objects from all delegations
        const allTimelockedStakedObjects =
            timelockedStakes?.flatMap((delegation) =>
                delegation.stakes.map((stake) => ({
                    objectId: stake.timelockedStakedIotaId,
                    content: stake,
                })),
            ) || [];

        // Only create transaction if there are items to collect
        if (timelockObjectIds.length === 0 && allTimelockedStakedObjects.length === 0) {
            return null;
        }

        const ptb = createCollectAllTimelocksTransaction({
            address: activeAddress,
            timelockObjectIds,
            timelockedStakedObjects: allTimelockedStakedObjects as never,
        });

        ptb.setSenderIfNotSet(activeAddress);
        return ptb;
    }, [activeAddress, timelockObjects, timelockedStakes]);

    // Dry run the transaction
    const {
        data: dryRunData,
        isError: isDryRunError,
        isPending: isDryRunLoading,
    } = useTransactionDryRun(activeAddress || undefined, transaction as Transaction);

    const { isPending: isTransactionDataPending } = useTransactionData(
        activeAddress || undefined,
        transaction as Transaction,
    );

    const summary = useTransactionSummary({
        transaction: dryRunData,
        currentAddress: activeAddress || undefined,
        recognizedPackagesList,
    });

    const isDryRunExecutionFailed = dryRunData?.effects.status.status === 'failure';
    const dryRunExecutionError = dryRunData?.effects.status.error;
    const dryRunExecutionSupportingText = dryRunExecutionError
        ? getUserFriendlyDryRunExecutionError(dryRunExecutionError)
        : undefined;

    const handleClose = () => {
        setOpen(false);
    };

    const handleApprove = async () => {
        if (!transaction) return;

        try {
            await signAndExecuteTransaction(
                {
                    transaction: transaction as Transaction,
                },
                {
                    onSuccess: () => {
                        toast.success('Collection completed successfully');
                        onSuccess?.();
                        handleClose();
                    },
                },
            );
        } catch (error) {
            toast.error('Failed to collect assets');
            // eslint-disable-next-line no-console
            console.error('Collection error:', error);
        }
    };

    const isLoading = isTimelocksLoading || isTimelockedStakesLoading || isDryRunLoading;
    const hasNoAssets = !transaction;

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="Collect All Assets" onClose={handleClose} titleCentered />
                <DialogBody>
                    {isLoading ? (
                        <div className="flex items-center justify-center p-10">
                            <LoadingIndicator />
                        </div>
                    ) : hasNoAssets ? (
                        <InfoBox
                            title="No Assets to Collect"
                            supportingText="You don't have any timelocks or timelock stakes available to collect."
                            type={InfoBoxType.Default}
                            style={InfoBoxStyle.Elevated}
                        />
                    ) : (
                        <div className="flex flex-col gap-md">
                            {isDryRunExecutionFailed && dryRunExecutionSupportingText && (
                                <InfoBox
                                    title={DRY_RUN_UI_ERROR_TITLE}
                                    supportingText={dryRunExecutionSupportingText}
                                    icon={<Warning />}
                                    type={InfoBoxType.Error}
                                    style={InfoBoxStyle.Elevated}
                                />
                            )}
                            {!isDryRunLoading && (!summary || isDryRunError) && (
                                <InfoBox
                                    title="Review the transaction"
                                    supportingText="Unexpected issue during the dry run. The transaction may not execute properly."
                                    icon={<Warning />}
                                    type={InfoBoxType.Default}
                                    style={InfoBoxStyle.Elevated}
                                />
                            )}
                            <TransactionSummary
                                isDryRun
                                isLoading={isDryRunLoading}
                                isError={isDryRunError}
                                summary={summary}
                                renderExplorerLink={ExplorerLinkHelper}
                            />
                            <GasFees
                                sender={activeAddress || undefined}
                                gasSummary={summary?.gas}
                                isEstimate
                                isError={isDryRunError}
                                isPending={isTransactionDataPending}
                                activeAddress={activeAddress || undefined}
                                renderExplorerLink={ExplorerLinkHelper}
                            />
                        </div>
                    )}
                </DialogBody>
                {!hasNoAssets && (
                    <div className="flex w-full flex-row justify-center gap-2 px-md--rs pb-md--rs pt-sm--rs">
                        <Button
                            onClick={handleClose}
                            fullWidth
                            text="Cancel"
                            type={ButtonType.Ghost}
                        />
                        <Button
                            onClick={handleApprove}
                            fullWidth
                            text="Approve"
                            type={ButtonType.Primary}
                            disabled={
                                isLoading || isDryRunError || isDryRunExecutionFailed || isExecuting
                            }
                            icon={isExecuting ? <LoadingIndicator /> : undefined}
                        />
                    </div>
                )}
            </DialogContent>
        </Dialog>
    );
}
