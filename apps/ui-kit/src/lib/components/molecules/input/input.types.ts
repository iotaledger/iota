// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ComponentProps } from 'react';
import { InputType } from '.';
import { NumericFormat } from 'react-number-format';

type InputElementProps = Omit<
    React.ComponentProps<'input'>,
    'type' | 'className' | 'ref' | 'value' | 'defaultValue'
>;

export type PasswordInputProps = {
    type: InputType.Password;
};

export type TextInputProps = {
    type: InputType.Text;
};

type NumericFormatProps = ComponentProps<typeof NumericFormat>;

export type NumberInputProps = {
    type: InputType.Number;
    /**
     * The pattern attribute specifies a regular expression that the input element's value is checked against.
     */
    pattern?: string;
    /**
     * onValueChange callback
     */
    onValueChange?: NumericFormatProps['onValueChange'];
    /**
     * The decimal scale
     */
    decimalScale?: NumericFormatProps['decimalScale'];
    /**
     * The thousand separator
     */
    thousandSeparator?: NumericFormatProps['thousandSeparator'];
    /**
     * Allow negative numbers
     */
    allowNegative?: NumericFormatProps['allowNegative'];
    /**
     * The suffix
     */
    suffix?: NumericFormatProps['suffix'];
    /**
     * The prefix
     */
    prefix?: NumericFormatProps['prefix'];
};

export type InputPropsByType = InputElementProps &
    (TextInputProps | NumberInputProps | PasswordInputProps);
