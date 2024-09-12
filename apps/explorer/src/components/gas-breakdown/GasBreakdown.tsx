// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Title } from '@iota/apps-ui-kit';
import {
    CoinFormat,
    type TransactionSummary,
    useFormatCoin,
    useResolveIotaNSName,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Heading, Text } from '@iota/ui';

import {
    AddressLink,
    CollapsibleCard,
    CollapsibleSection,
    CopyToClipboard,
    DescriptionItem,
    Divider,
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
        <div className="flex flex-wrap gap-1">
            <div className="flex flex-wrap items-center gap-1">
                <Text variant="pBody/medium" color="steel-darker">
                    {formattedAmount}
                </Text>
                <Text variant="subtitleSmall/medium" color="steel-darker">
                    {symbol}
                </Text>
            </div>

            <div className="flex flex-wrap items-center text-body font-medium text-steel">
                ({BigInt(amount)?.toLocaleString()}
                <div className="ml-0.5 text-subtitleSmall font-medium text-steel">nano</div>)
            </div>
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
                {isSponsored && owner && (
                    <div className="mb-4 flex items-center gap-2 rounded-xl bg-iota/10 px-3 py-2">
                        <Text variant="pBody/medium" color="steel-darker">
                            Paid by
                        </Text>
                        <AddressLink label={iotansDomainName || undefined} address={owner} />
                    </div>
                )}

                <div className="flex flex-col gap-3">
                    <Divider />

                    <div className="flex flex-col gap-2 md:flex-row md:gap-10">
                        <div className="w-full flex-shrink-0 md:w-40">
                            <Text variant="pBody/semibold" color="steel-darker">
                                Gas Payment
                            </Text>
                        </div>
                        {gasPayment?.length ? (
                            <GasPaymentLinks objectIds={gasPayment.map((gas) => gas.objectId)} />
                        ) : null}
                    </div>

                    <DescriptionItem
                        align="start"
                        title={<Text variant="pBody/semibold">Gas Budget</Text>}
                    >
                        {gasBudget ? <GasAmount amount={BigInt(gasBudget)} /> : null}
                    </DescriptionItem>
                </div>

                <div className="mt-4 flex flex-col gap-3">
                    <Divider />

                    <DescriptionItem
                        align="start"
                        title={<Text variant="pBody/semibold">Computation Fee</Text>}
                    >
                        <GasAmount amount={Number(gasUsed?.computationCost)} />
                    </DescriptionItem>

                    <DescriptionItem
                        align="start"
                        title={<Text variant="pBody/semibold">Storage Fee</Text>}
                    >
                        <GasAmount amount={Number(gasUsed?.storageCost)} />
                    </DescriptionItem>

                    <DescriptionItem
                        align="start"
                        title={<Text variant="pBody/semibold">Storage Rebate</Text>}
                    >
                        <div className="-ml-1.5 min-w-0 flex-1 leading-none">
                            <GasAmount amount={-Number(gasUsed?.storageRebate)} />
                        </div>
                    </DescriptionItem>
                </div>

                <div className="mt-6 flex flex-col gap-6">
                    <Divider />

                    <DescriptionItem
                        align="start"
                        title={<Text variant="pBody/semibold">Gas Price</Text>}
                    >
                        <GasAmount amount={BigInt(gasPrice)} />
                    </DescriptionItem>
                </div>
            </CollapsibleSection>
        </CollapsibleCard>
    );
}
