// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TextFieldType } from './text-field.enums';

type TextFieldTypePasswordProps = {
    type: TextFieldType.Password;
};

type TextFieldTypeTextProps = {
    type: TextFieldType.Text;
};

export type TextFieldTypeTextAreaProps = {
    type: TextFieldType.TextArea;
    /**
     * Show a button to hide the content of the textarea.
     */
    hideContentToggle?: boolean;
    /**
     * The number of rows to show in the textarea
     */
    rows?: number;
    /**
     * The number of columns to show in the textarea
     */
    cols?: number;
    /**
     * How many 'hidden' rows are displayed.
     */
    hiddenRows?: number;
};

export type TextFieldPropsByType =
    | TextFieldTypePasswordProps
    | TextFieldTypeTextProps
    | TextFieldTypeTextAreaProps;
