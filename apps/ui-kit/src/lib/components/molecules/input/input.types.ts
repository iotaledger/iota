// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ComponentProps } from 'react';
import type { InputType } from '.';
import type { NumericFormat } from 'react-number-format';

type InputElementProps = Omit<
    React.ComponentProps<'input'>,
    'type' | 'className' | 'ref' | 'value' | 'defaultValue'
>;

type GenericInputProps = {
    type: InputType.Text | InputType.Password | InputType.Number;
};

type NumericFormatProps = ComponentProps<typeof NumericFormat>;

export type NumericFormatInputProps = {
    type: InputType.NumericFormat;
    suffix?: NumericFormatProps['suffix'];
    prefix?: NumericFormatProps['prefix'];
    decimalScale?: NumericFormatProps['decimalScale'];
    allowNegative?: NumericFormatProps['allowNegative'];
    thousandSeparator?: NumericFormatProps['thousandSeparator'];
    onValueChange?: NumericFormatProps['onValueChange'];
};

export type InputPropsByType = InputElementProps & (NumericFormatInputProps | GenericInputProps);
