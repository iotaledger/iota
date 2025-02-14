// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Accordion,
    AccordionHeader,
    AccordionContent,
    KeyValueInfo,
    Divider,
} from '@iota/apps-ui-kit';
import { type IotaEvent } from '@iota/iota-sdk/client';
import { formatAddress, parseStructTag } from '@iota/iota-sdk/utils';
import { TriangleDown } from '@iota/apps-ui-icons';
import clsx from 'clsx';
import { useState } from 'react';
import { SyntaxHighlighter } from '~/components';
import { ObjectLink } from '~/components/ui';

function Event({ event, divider }: { event: IotaEvent; divider: boolean }): JSX.Element {
    const [open, setOpen] = useState(false);
    const { address, module, name } = parseStructTag(event.type);
    const objectLinkLabel = [formatAddress(address), module, name].join('::');

    return (
        <div className="w-full">
            <div className="flex flex-col gap-3">
                <KeyValueInfo
                    keyText="Type"
                    value={objectLinkLabel}
                    copyText={objectLinkLabel}
                    fullwidth
                    isTruncated
                />

                <KeyValueInfo
                    keyText="Event Emitter"
                    value={
                        <ObjectLink
                            objectId={event.packageId}
                            queryStrings={{ module: event.transactionModule }}
                            label={`${formatAddress(event.packageId)}::${event.transactionModule}`}
                        />
                    }
                    copyText={event.packageId}
                    fullwidth
                    isTruncated
                />
                <Accordion hideBorder>
                    <AccordionHeader hideArrow isExpanded={open} onToggle={() => setOpen(!open)}>
                        <div className="flex w-full flex-row justify-between gap-xxxs pl-xxs text-neutral-40 dark:text-neutral-60">
                            <span className="text-body-md">
                                {open ? 'Hide' : 'View'} Event Data
                            </span>

                            <TriangleDown
                                className={clsx(
                                    'h-5 w-5',
                                    open
                                        ? 'rotate-0 transition-transform ease-linear'
                                        : '-rotate-90 transition-transform ease-linear',
                                )}
                            />
                        </div>
                    </AccordionHeader>
                    <AccordionContent isExpanded={open}>
                        <div className="mt-md">
                            <SyntaxHighlighter
                                code={JSON.stringify(event, null, 2)}
                                language="json"
                            />
                        </div>
                    </AccordionContent>
                </Accordion>
            </div>

            {divider && (
                <div className="my-6">
                    <Divider />
                </div>
            )}
        </div>
    );
}

interface EventsProps {
    events: IotaEvent[];
}

export function Events({ events }: EventsProps) {
    return (
        <div className="flex flex-wrap gap-lg px-md--rs py-md md:py-md">
            {events.map((event, index) => (
                <Event key={event.type} event={event} divider={index !== events.length - 1} />
            ))}
        </div>
    );
}
