// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useField, useFormikContext } from 'formik';
import { ButtonHtmlType, ButtonPill, Input, InputType } from '@iota/apps-ui-kit';

interface InputWithActionProps {
    disabled?: boolean;
    name: string;
    min?: number;
    placeholder?: string;
}

export function InputWithAction({
    disabled = false,
    min,
    placeholder,
    name,
    ...props
}: InputWithActionProps) {
    const [field, meta] = useField(name);
    const form = useFormikContext();
    const { isSubmitting } = form;
    const isInputDisabled = isSubmitting || disabled;
    const isActionDisabled = isInputDisabled || meta?.initialValue === meta?.value || !!meta?.error;

    return (
        <>
            <Input
                type={InputType.Text}
                disabled={isInputDisabled}
                placeholder={placeholder}
                errorMessage={meta?.error}
                min={min}
                {...field}
                {...props}
                trailingElement={
                    <ButtonPill htmlType={ButtonHtmlType.Submit} disabled={isActionDisabled}>
                        Save
                    </ButtonPill>
                }
            />
        </>
    );
}
