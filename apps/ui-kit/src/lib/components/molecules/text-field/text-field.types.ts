// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TextFieldType } from './text-field.enums';

type NumberTextFieldProps = {
    type: TextFieldType.Number;
    /**
     * The minimum accepted value for the input field
     */
    min?: number;
    /**
     * The maximum accepted value for the input field
     */
    max?: number;
    /**
     * The increment/decrement step for the input field
     */
    step?: number;
};

type PasswordTextFieldProps = {
    type: TextFieldType.Password;
    /**
     * Wheter the password toggle button should be hidden
     */
    hidePasswordToggle?: boolean;
};

type EmailTextFieldProps = {
    type: TextFieldType.Email;
};

type TextTextFieldProps = {
    type: TextFieldType.Text;
};

export type TextFieldPropsByType =
    | NumberTextFieldProps
    | PasswordTextFieldProps
    | EmailTextFieldProps
    | TextTextFieldProps;
