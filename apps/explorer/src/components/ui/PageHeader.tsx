// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Badge,
    BadgeType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Panel,
    Placeholder,
} from '@iota/apps-ui-kit';
import { Copy, Warning } from '@iota/apps-ui-icons';
import { onCopySuccess } from '~/lib/utils';

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

export function PageHeader({
    title,
    subtitle,
    type,
    error,
    loading,
    after,
    status,
}: PageHeaderProps): JSX.Element {
    return (
        <Panel>
            <div className="flex w-full items-center p-md--rs">
                <div className="flex w-full flex-col items-start justify-between gap-sm md:flex-row md:items-center">
                    <div className="flex w-full flex-col gap-xxs md:w-3/4">
                        {loading ? (
                            <div className="flex w-full flex-col gap-xs">
                                {new Array(2).fill(0).map((_, index) => (
                                    <Placeholder
                                        key={index}
                                        width={index === 0 ? 'w-1/2' : 'w-2/3'}
                                    />
                                ))}
                            </div>
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
                                        <span
                                            className="break-all text-body-ds-lg"
                                            data-testid="heading-object-id"
                                        >
                                            {title}
                                        </span>
                                        <Copy
                                            onClick={onCopySuccess}
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
                                            type={InfoBoxType.Error}
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
