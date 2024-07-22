// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TextFieldType } from './text-field.enums';

type PasswordTextFieldProps = {
    type: TextFieldType.Password;
    /**
     * Whether the password toggle button should be hidden
     */
    hidePasswordToggle?: boolean;
};

type TextTextFieldProps = {
    type: TextFieldType.Text;
};

export type TextFieldPropsByType = PasswordTextFieldProps | TextTextFieldProps;
