// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InputFieldType } from './input-field.enums';

type InputFieldTypePasswordProps = {
    type: InputFieldType.Password;
    /**
     * Shows toggle button to show/hide the content of the input field
     */
    isVisibilityToggleEnabled?: boolean;
};

type InputFieldTypeTextProps = {
    type: InputFieldType.Text;
};

type InputFieldTypeNumberProps = {
    type: InputFieldType.Number;
};

export type InputFieldPropsByType =
    | InputFieldTypePasswordProps
    | InputFieldTypeTextProps
    | InputFieldTypeNumberProps;
