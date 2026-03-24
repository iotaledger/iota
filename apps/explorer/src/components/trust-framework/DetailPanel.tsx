// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TriangleDown } from '@iota/apps-ui-icons';
import { Accordion, AccordionContent, AccordionHeader } from '@iota/apps-ui-kit';
import { clsx } from 'clsx';
import { type ReactNode, useState } from 'react';

interface DetailPanelProps {
    panelContent: ReactNode;
    headerContent?: ReactNode;
    hideBorder?: boolean;
}

export function DetailPanel({ panelContent, headerContent }: DetailPanelProps): JSX.Element {
    const [open, setOpen] = useState(false);
    return (
        <Accordion hideBorder>
            <AccordionHeader hideArrow isExpanded={open} onToggle={() => setOpen(!open)}>
                <div className="flex w-full flex-row items-center justify-between px-md--rs">
                    <div className="flex flex-row gap-xxxs text-iota-neutral-40 dark:text-iota-neutral-60">
                        <span className="text-body-md">Object</span>

                        <TriangleDown
                            className={clsx(
                                'h-5 w-5',
                                open
                                    ? 'rotate-0 transition-transform ease-linear'
                                    : '-rotate-90 transition-transform ease-linear',
                            )}
                        />
                    </div>
                    <div className="flex flex-row items-center gap-xxs overflow-hidden truncate pr-xxs">
                        {headerContent}
                    </div>
                </div>
            </AccordionHeader>
            <AccordionContent isExpanded={open}>{panelContent}</AccordionContent>
        </Accordion>
    );
}
