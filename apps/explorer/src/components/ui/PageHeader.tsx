// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeType, InfoBox, InfoBoxStyle, InfoBoxType, Panel } from '@iota/apps-ui-kit';
import { Copy, Flag, Globe, Warning } from '@iota/ui-icons';
import { Placeholder } from '@iota/ui';
import { useCopyToClipboard } from '@iota/core';
import toast from 'react-hot-toast';

type PageHeaderType = 'Transaction' | 'Checkpoint' | 'Address' | 'Object' | 'Package';

export interface PageHeaderProps {
    title: string;
    subtitle?: string | null;
    type: PageHeaderType;
    status?: 'success' | 'failure';
    after?: React.ReactNode;
    error?: string;
    loading?: boolean;
}

const TYPE_TO_ICON: Record<PageHeaderType, React.ComponentType<{ className?: string }> | null> = {
    Transaction: null,
    Checkpoint: Flag,
    Object: null,
    Package: null,
    Address: Globe,
};

export function PageHeader({
    title,
    subtitle,
    type,
    error,
    loading,
    after,
    status,
}: PageHeaderProps): JSX.Element {
    const copyToClipBoard = useCopyToClipboard(() => toast.success('Copied'));
    const Icon = TYPE_TO_ICON[type];

    const handleCopy = async () => {
        await copyToClipBoard(title);
    };

    return (
        <Panel>
            <div className="flex w-full items-center p-md--rs">
                <div className="flex w-full flex-col items-start justify-between gap-sm md:flex-row md:items-center">
                    <div className="flex w-full flex-col gap-xxs md:w-3/4">
                        {loading ? (
                            <Placeholder rounded="xl" width="50%" height="10px" />
                        ) : (
                            <>
                                {type && (
                                    <div className="flex flex-row items-center gap-xxs">
                                        <span className="text-headline-sm text-neutral-10 dark:text-neutral-92">
                                            {type}
                                        </span>
                                        {status && (
                                            <Badge
                                                label={status}
                                                type={
                                                    status === 'success'
                                                        ? BadgeType.PrimarySoft
                                                        : BadgeType.Neutral
                                                }
                                            />
                                        )}
                                    </div>
                                )}
                                {title && (
                                    <div className="flex items-center gap-xxs text-neutral-40 dark:text-neutral-60">
                                        {Icon && <Icon className="shrink-0" />}
                                        <span className="break-all text-body-ds-lg">{title}</span>
                                        <Copy
                                            onClick={handleCopy}
                                            className="shrink-0 cursor-pointer"
                                        />
                                    </div>
                                )}
                                {subtitle && (
                                    <span className="pt-sm text-body-md text-neutral-40 dark:text-neutral-60">
                                        {subtitle}
                                    </span>
                                )}
                                {error && (
                                    <div className="mt-xs--rs flex">
                                        <InfoBox
                                            title={error}
                                            icon={<Warning />}
                                            type={InfoBoxType.Warning}
                                            style={InfoBoxStyle.Elevated}
                                        />
                                    </div>
                                )}
                            </>
                        )}
                    </div>
                    {after && <div className="w-1/2 sm:w-1/4">{after}</div>}
                </div>
            </div>
        </Panel>
    );
}
