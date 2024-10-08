// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaCallArg } from '@iota/iota-sdk/client';
import { Text } from '@iota/ui';
import { ProgrammableTxnBlockCard } from '~/components';
import { AddressLink, CollapsibleSection, ObjectLink } from '~/components/ui';

const REGEX_NUMBER = /^\d+$/;

interface InputsCardProps {
    inputs: IotaCallArg[];
}

export function InputsCard({ inputs }: InputsCardProps): JSX.Element | null {
    if (!inputs?.length) {
        return null;
    }

    const expandableItems = inputs.map((input, index) => (
        <CollapsibleSection key={index} title={`Input ${index}`} defaultOpen>
            <div
                data-testid="inputs-card-content"
                className="flex flex-col gap-2 px-md pb-lg pt-xs"
            >
                {Object.entries(input).map(([key, value]) => {
                    let renderValue;
                    const stringValue = String(value);

                    if (key === 'mutable') {
                        renderValue = String(value);
                    } else if (key === 'objectId') {
                        renderValue = <ObjectLink objectId={stringValue} />;
                    } else if (
                        'valueType' in input &&
                        'value' in input &&
                        input.valueType === 'address' &&
                        key === 'value'
                    ) {
                        renderValue = <AddressLink address={stringValue} />;
                    } else if (REGEX_NUMBER.test(stringValue)) {
                        const bigNumber = BigInt(stringValue);
                        renderValue = bigNumber.toLocaleString();
                    } else {
                        renderValue = stringValue;
                    }

                    return (
                        <div key={key} className="flex items-start justify-between">
                            <Text variant="pBody/medium" color="steel-dark">
                                {key}
                            </Text>

                            <div className="max-w-[66%] break-all text-right">
                                <Text variant="pBody/medium" color="steel-darker">
                                    {renderValue}
                                </Text>
                            </div>
                        </div>
                    );
                })}
            </div>
        </CollapsibleSection>
    ));

    return (
        <ProgrammableTxnBlockCard
            initialClose
            items={expandableItems}
            itemsLabel={inputs.length > 1 ? 'Inputs' : 'Input'}
            count={inputs.length}
        />
    );
}
