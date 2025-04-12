// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Accordion, AccordionContent, Title, Divider } from '@iota/apps-ui-kit';
import {
    CoinFormat,
    type TransactionSummaryType,
    useCopyToClipboard,
    useFormatCoin,
    toast,
} from '@iota/core';
import { Copy } from '@iota/apps-ui-icons';
import { AddressLink, CollapsibleCard, ObjectLink } from '~/components/ui';
import { Fragment } from 'react';

interface GasProps {
    amount?: bigint | number | string;
    burnedAmount?: bigint | number | string | undefined;
}

function GasAmount({ amount, burnedAmount }: GasProps): JSX.Element | null {
    const [formattedAmount, symbol] = useFormatCoin({ balance: amount, format: CoinFormat.FULL });
    const [formattedBurnedAmount, burnedSymbol] = useFormatCoin({
        balance: burnedAmount,
        format: CoinFormat.FULL,
    });

    if (!amount) {
        return null;
    }

    return (
        <div className="flex flex-wrap items-center gap-xxs">
            <span className="text-label-lg text-neutral-40 dark:text-neutral-60">
                {formattedAmount} {symbol}
            </span>
            <span className="flex flex-wrap items-center text-body-md font-medium text-neutral-70">
                {BigInt(amount)?.toLocaleString()} (nano)
            </span>
            {!!burnedAmount && (
                <>
                    <span className="text-label-md text-neutral-40 dark:text-neutral-60">
                        Burnt: {formattedBurnedAmount} {burnedSymbol}
                    </span>
                    <span className="flex flex-wrap items-center text-body-sm font-medium text-neutral-70">
                        {BigInt(burnedAmount)?.toLocaleString()} (nano)
                    </span>
                </>
            )}
        </div>
    );
}

function GasPaymentLinks({ objectIds }: { objectIds: string[] }): JSX.Element {
    const copyToClipBoard = useCopyToClipboard(() => toast('Copied'));

    const handleCopy = async (objectId: string) => {
        await copyToClipBoard(objectId);
    };

    return (
        <div className="flex max-h-20 min-h-[20px] flex-wrap items-center gap-x-4 gap-y-2 overflow-y-auto">
            {objectIds.map((objectId, index) => (
                <div key={index} className="flex items-center gap-x-1.5">
                    <ObjectLink objectId={objectId} />
                    <Copy
                        className="shrink-0 cursor-pointer text-neutral-70"
                        onClick={() => handleCopy(objectId)}
                    />
                </div>
            ))}
        </div>
    );
}

function GasInfo({ label, info }: { label: string; info?: React.ReactNode }) {
    return (
        <div className="flex flex-col gap-2 md:flex-row md:gap-10">
            <span className="w-full flex-shrink-0 text-label-lg text-neutral-40 md:w-40 dark:text-neutral-60">
                {label}
            </span>
            {info ? (
                info
            ) : (
                <span className="text-label-lg text-neutral-40 md:w-40 dark:text-neutral-60">
                    --
                </span>
            )}
        </div>
    );
}

interface GasBreakdownProps {
    summary?: TransactionSummaryType | null;
}

interface GasData {
    label: string;
    info: React.ReactNode;
    divider?: true;
}

export function GasBreakdown({ summary }: GasBreakdownProps): JSX.Element | null {
    const gasData = summary?.gas;

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

    const GAS_SECTIONS: GasData[] = [
        {
            label: 'Gas Payment',
            info: gasPayment?.length && (
                <GasPaymentLinks objectIds={gasPayment.map((gas) => gas.objectId)} />
            ),
            divider: true,
        },
        {
            label: 'Gas Budget',
            info: gasBudget && <GasAmount amount={BigInt(gasBudget)} />,
            divider: true,
        },
        {
            label: 'Gas Price',
            info: gasPrice && <GasAmount amount={BigInt(gasPrice)} />,
            divider: true,
        },
        {
            label: 'Computation Fee',
            info: gasUsed?.computationCost && (
                <GasAmount
                    amount={Number(gasUsed.computationCost)}
                    burnedAmount={Number(gasUsed.computationCostBurned)}
                />
            ),
        },
        {
            label: 'Storage Fee',
            info: gasUsed?.storageCost && <GasAmount amount={Number(gasUsed.storageCost)} />,
        },
        {
            label: 'Storage Rebate',
            info: gasUsed?.storageRebate && <GasAmount amount={-Number(gasUsed.storageRebate)} />,
            divider: true,
        },
        {
            label: 'Total Gas Fee',
            info: <GasAmount amount={totalGas} />,
        },
    ];

    return (
        <CollapsibleCard
            collapsible
            render={({ isOpen }) => <Title title="Gas & Storage Fee" />}
            hideBorder
        >
            <div className="px-md--rs pb-lg pt-xs">
                <Accordion hideBorder>
                    <AccordionContent isExpanded>
                        <div className="flex flex-col gap-xs">
                            {isSponsored && owner && (
                                <div className="flex items-center gap-md rounded-lg bg-neutral-92 p-xs dark:bg-neutral-12">
                                    <span className="text-label-lg text-neutral-40 dark:text-neutral-60">
                                        Paid by
                                    </span>
                                    <AddressLink label={undefined} address={owner} />
                                </div>
                            )}

                            <div className="flex flex-col gap-4 py-2">
                                {GAS_SECTIONS.filter((section) => !!section.info).map(
                                    (section, index) => (
                                        <Fragment key={index}>
                                            <GasInfo label={section.label} info={section.info} />
                                            {section.divider && <Divider />}
                                        </Fragment>
                                    ),
                                )}
                            </div>
                        </div>
                    </AccordionContent>
                </Accordion>
            </div>
        </CollapsibleCard>
    );
}
