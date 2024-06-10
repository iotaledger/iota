// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '@/components';

interface SelectValidatorViewProps {
    validators: string[];
    onSelect: (validator: string) => void;
}

function SelectValidatorView({ validators, onSelect }: SelectValidatorViewProps): JSX.Element {
    return (
        <div>
            <h2>Select Validator</h2>
            <div className="flex flex-col items-start gap-2">
                {validators.map((validator) => (
                    <Button key={validator} onClick={() => onSelect(validator)}>
                        {validator}
                    </Button>
                ))}
            </div>
        </div>
    );
}

export default SelectValidatorView;
