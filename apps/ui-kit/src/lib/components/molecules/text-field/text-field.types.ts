// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TextFieldType } from './text-field.enums';

type TextFieldTypePasswordProps = {
    type: TextFieldType.Password;
    /**
     * Shows toggle button to show/hide the content of the input field
     */
    isVisibilityToggleEnabled?: boolean;
};

type TextFieldTypeTextProps = {
    type: TextFieldType.Text;
};

export type TextFieldPropsByType = TextFieldTypePasswordProps | TextFieldTypeTextProps;
