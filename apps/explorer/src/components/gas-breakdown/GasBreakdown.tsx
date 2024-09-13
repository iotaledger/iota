// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Divider, Title } from '@iota/apps-ui-kit';
import {
    CoinFormat,
    type TransactionSummary,
    useFormatCoin,
    useResolveIotaNSName,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import {
    AddressLink,
    CollapsibleCard,
    CollapsibleSection,
    CopyToClipboard,
    ObjectLink,
} from '~/components/ui';

interface GasProps {
    amount?: bigint | number | string;
}

function GasAmount({ amount }: GasProps): JSX.Element | null {
    const [formattedAmount, symbol] = useFormatCoin(amount, IOTA_TYPE_ARG, CoinFormat.FULL);

    if (!amount) {
        return null;
    }

    return (
        <div className="flex flex-wrap gap-xxs">
            <span className="text-label-lg text-neutral-40 dark:text-neutral-60">
                {formattedAmount} {symbol}
            </span>
            <span className="flex flex-wrap items-center text-body font-medium text-neutral-70">
                {BigInt(amount)?.toLocaleString()} nano
            </span>
        </div>
    );
}

function TotalGasAmount({ amount }: GasProps): JSX.Element | null {
    const [formattedAmount, symbol] = useFormatCoin(amount, IOTA_TYPE_ARG, CoinFormat.FULL);

    if (!amount) {
        return null;
    }

    return (
        <div className="flex w-full flex-row items-center justify-between gap-md px-md--rs pt-xs">
            <div className="flex flex-col items-start gap-xxs">
                <span className="text-body-lg text-neutral-10 dark:text-neutral-92">
                    {formattedAmount}
                </span>
                <span className="text-label-lg text-neutral-40 dark:text-neutral-60">{symbol}</span>
            </div>

            <div className="flex flex-col items-start gap-xxs">
                <span className="text-body-lg text-neutral-10 dark:text-neutral-92">
                    {BigInt(amount)?.toLocaleString()}
                </span>
                <span className="text-label-lg text-neutral-40 dark:text-neutral-60">nano</span>
            </div>
        </div>
    );
}

function GasPaymentLinks({ objectIds }: { objectIds: string[] }): JSX.Element {
    return (
        <div className="flex max-h-20 min-h-[20px] flex-wrap items-center gap-x-4 gap-y-2 overflow-y-auto">
            {objectIds.map((objectId, index) => (
                <div key={index} className="flex items-center gap-x-1.5">
                    <ObjectLink objectId={objectId} />
                    <CopyToClipboard size="sm" copyText={objectId} />
                </div>
            ))}
        </div>
    );
}

interface GasBreakdownProps {
    summary?: TransactionSummary | null;
}

export function GasBreakdown({ summary }: GasBreakdownProps): JSX.Element | null {
    const gasData = summary?.gas;
    const { data: iotansDomainName } = useResolveIotaNSName(gasData?.owner);

    if (!gasData) {
        return null;
    }

    const gasPayment = gasData.payment;
    const gasUsed = gasData.gasUsed;
    const gasPrice = gasData.price || 1;
    const gasBudget = gasData.budget;
    const totalGas = gasData.totalGas;
    const owner = gasData.owner;
    const isSponsored = gasData.isSponsored;

    return (
        <CollapsibleCard
            collapsible
            render={({ isOpen }) => (
                <div className="flex w-full flex-col gap-2">
                    <Title title="Gas & Storage Fee" />
                    <TotalGasAmount amount={totalGas} />
                </div>
            )}
        >
            <CollapsibleSection hideBorder>
                <div className="flex flex-col gap-xs">
                    {isSponsored && owner && (
                        <div className="flex items-center gap-md rounded-lg bg-neutral-92 p-xs dark:bg-neutral-12">
                            <span className="text-label-lg text-neutral-40 dark:text-neutral-60">
                                Paid by
                            </span>
                            <AddressLink label={iotansDomainName || undefined} address={owner} />
                        </div>
                    )}
                    <div className="flex flex-col gap-3">
                        <Divider />
                        <div className="flex flex-col gap-2 md:flex-row md:gap-10">
                            <span className="w-full flex-shrink-0 text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                                Gas Payment
                            </span>
                            {gasPayment?.length ? (
                                <GasPaymentLinks
                                    objectIds={gasPayment.map((gas) => gas.objectId)}
                                />
                            ) : (
                                <span className="text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                                    --
                                </span>
                            )}
                        </div>
                        <div className="flex flex-col gap-2 md:flex-row md:gap-10">
                            <span className="w-full flex-shrink-0 text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                                Gas Budget
                            </span>
                            {gasBudget ? (
                                <GasAmount amount={BigInt(gasBudget)} />
                            ) : (
                                <span className="text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                                    --
                                </span>
                            )}
                        </div>
                    </div>
                    <div className="mt-4 flex flex-col gap-3">
                        <Divider />
                        <div className="flex flex-col gap-2 md:flex-row md:gap-10">
                            <span className="w-full flex-shrink-0 text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                                Computation Fee
                            </span>
                            {gasUsed?.computationCost ? (
                                <GasAmount amount={Number(gasUsed?.computationCost)} />
                            ) : (
                                <span className="text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                                    --
                                </span>
                            )}
                        </div>
                        <div className="flex flex-col gap-2 md:flex-row md:gap-10">
                            <span className="w-full flex-shrink-0 text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                                Storage Fee
                            </span>
                            {gasUsed?.storageCost ? (
                                <GasAmount amount={Number(gasUsed?.storageCost)} />
                            ) : (
                                <span className="text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                                    --
                                </span>
                            )}
                        </div>
                        <div className="flex flex-col gap-2 md:flex-row md:gap-10">
                            <span className="w-full flex-shrink-0 text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                                Storage Rebate
                            </span>
                            {gasUsed?.storageRebate ? (
                                <GasAmount amount={-Number(gasUsed?.storageRebate)} />
                            ) : (
                                <span className="text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                                    --
                                </span>
                            )}
                        </div>
                    </div>
                    <div className="mt-6 flex flex-col gap-6">
                        <Divider />
                        <div className="flex flex-col gap-2 md:flex-row md:gap-10">
                            <span className="w-full flex-shrink-0 text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                                Gas Price
                            </span>
                            {gasPrice ? (
                                <GasAmount amount={BigInt(gasPrice)} />
                            ) : (
                                <span className="text-label-lg text-neutral-40 dark:text-neutral-60 md:w-40">
                                    --
                                </span>
                            )}
                        </div>
                    </div>
                </div>
            </CollapsibleSection>
        </CollapsibleCard>
    );
}
