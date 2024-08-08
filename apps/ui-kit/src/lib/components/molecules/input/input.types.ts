// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InputType } from './input.enums';

type InputTypePasswordProps = {
    type: InputType.Password;
    /**
     * Shows toggle button to show/hide the content of the input
     */
    isVisibilityToggleEnabled?: boolean;
};

type InputTypeTextProps = {
    type: InputType.Text;
};

type InputTypeNumberProps = {
    type: InputType.Number;
};

export type InputPropsByType = InputTypePasswordProps | InputTypeTextProps | InputTypeNumberProps;
