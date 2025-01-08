// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button } from '@/lib';
import { Showcase } from '@/storybook/blocks';
import { VARIABLE_GAP_CLASSES, VARIABLE_PADDING_CLASSES } from '@/storybook/constants';

export function VariableSpacingShowcase() {
    return (
        <div className="flex flex-col gap-10">
            <p className="text-headline-sm text-neutral-10">
                The variable spacing changes based on the screen size.
            </p>

            <DocumentedBlock title="Gap">
                {VARIABLE_GAP_CLASSES.map((value) => (
                    <Showcase key={value} title={value}>
                        <div className={`flex flex-row ${value}`}>
                            <Button text="Button 1" />
                            <Button text="Button 2" />
                        </div>
                    </Showcase>
                ))}
            </DocumentedBlock>

            <DocumentedBlock title="Padding">
                {VARIABLE_PADDING_CLASSES.map((value) => (
                    <Showcase key={value} title={value}>
                        <div className={`flex flex-row ${value}`}>
                            <Button text="Button 1" />
                        </div>
                    </Showcase>
                ))}
            </DocumentedBlock>
        </div>
    );
}

function DocumentedBlock({ title, children }: React.PropsWithChildren<{ title: string }>) {
    return (
        <div className="flex flex-col gap-y-4">
            <h2 className="text-headline-md text-neutral-10">{title}</h2>
            <div className="flex flex-row flex-wrap gap-10">{children}</div>
        </div>
    );
}
