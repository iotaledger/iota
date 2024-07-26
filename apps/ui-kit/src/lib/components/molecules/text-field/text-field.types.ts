// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TextFieldType } from './text-field.enums';

type TextFieldTypePasswordProps = {
    type: TextFieldType.Password;
};

type TextFieldTypeTextProps = {
    type: TextFieldType.Text;
};

export type TextFieldPropsByType = TextFieldTypePasswordProps | TextFieldTypeTextProps;
