// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode } from 'react';
import { useFormContext } from 'react-hook-form';

import { FormLabel } from './FormLabel';
import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { Exclamation } from '@iota/ui-icons';

interface FormFieldProps {
    name: string;
    label?: ReactNode;
    children: ReactNode;
}

export function FormField({ children, name, label }: FormFieldProps) {
    const { getFieldState, formState } = useFormContext();
    const state = getFieldState(name, formState);

    return (
        <div className="flex w-full flex-col gap-2.5">
            {label ? <FormLabel label={label}>{children}</FormLabel> : children}
            {state.error && (
                <InfoBox
                    type={InfoBoxType.Default}
                    supportingText={state.error.message}
                    icon={<Exclamation />}
                    style={InfoBoxStyle.Elevated}
                />
            )}
        </div>
    );
}

export default FormField;
