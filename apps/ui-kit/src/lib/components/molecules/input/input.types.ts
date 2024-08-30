// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InputType } from '.';

export type TextInputProps = {
    type: InputType.Text | InputType.Password;
};

export type NumberInputProps = {
    type: InputType.Number;
    /**
     * The pattern attribute specifies a regular expression that the input element's value is checked against.
     */
    pattern?: string;
    /**
     * If the input should allow negative numbers
     */
    allowNegative?: boolean;
    /**
     * If the input should allow decimals
     */
    decimals?: boolean;
    /**
     * The suffix to be shown after the input
     */
    suffix?: string;
    /**
     * The prefix to be shown before the input
     */
    prefix?: string;
    /**
     * On value change callback
     */
    onValueChange?: (value: string) => void;
};

export type InputPropsByType = TextInputProps | NumberInputProps;
