// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ComponentProps } from 'react';
import { Input } from '@iota/apps-ui-kit';
import { useField, useFormikContext } from 'formik';

interface FormInputWithFormixProps {
    name: string;
    renderAction?: (isDisabled: boolean | undefined) => React.JSX.Element;
}

export function FormInput({
    type,
    renderAction,
    ...props
}: Exclude<ComponentProps<typeof Input>, 'trailingElement'> & FormInputWithFormixProps) {
    const [field, meta, { setTouched }] = useField(props.name);
    const form = useFormikContext();

    const { isSubmitting } = form;
    const isInputDisabled = isSubmitting || props.disabled;

    const isActionButtonDisabled =
        isInputDisabled || meta?.initialValue === meta?.value || !!meta?.error;
    const errorMessage = meta?.error && meta.touched ? meta.error : undefined;

    return (
        <Input
            {...props}
            type={type}
            disabled={isInputDisabled}
            errorMessage={errorMessage}
            amountCounter={!errorMessage ? props.amountCounter : undefined}
            onFocus={(e) => {
                setTouched(true);
                props.onFocus?.(e);
            }}
            {...field}
            trailingElement={renderAction?.(isActionButtonDisabled)}
        />
    );
}
