// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Input, InputType, type InputProps, type NumberInputProps } from '@iota/apps-ui-kit';
import { useField, useFormikContext } from 'formik';

interface FormInputWithFormixProps {
    name: string;
    renderAction?: (isDisabled: boolean | undefined) => React.JSX.Element;
    decimals?: boolean;
}

export function FormInput({ renderAction, ...props }: InputProps & FormInputWithFormixProps) {
    const [field, meta] = useField(props.name);
    const form = useFormikContext();

    const { isSubmitting } = form;
    const isInputDisabled = isSubmitting || props.disabled;

    const isActionButtonDisabled =
        isInputDisabled || meta?.initialValue === meta?.value || !!meta?.error;
    const errorMessage = meta?.error && meta.touched ? meta.error : undefined;

    const numericPropsOnly: Partial<NumberInputProps> = {
        decimalScale: props.decimals ? undefined : 0,
        thousandSeparator: true,
        onValueChange: (values) => form.setFieldValue(props.name, values.value),
    };

    return (
        <Input
            {...props}
            {...field}
            defaultValue={meta.initialValue}
            disabled={isInputDisabled}
            errorMessage={errorMessage}
            amountCounter={!errorMessage ? props.amountCounter : undefined}
            trailingElement={renderAction?.(isActionButtonDisabled)}
            {...(props.type === InputType.Number ? numericPropsOnly : {})}
        />
    );
}
