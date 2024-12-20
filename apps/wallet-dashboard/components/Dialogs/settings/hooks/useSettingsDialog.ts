// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { SettingsDialogView } from '../enums';

export function useSettingsDialog() {
    const [settingsDialogView, setSettingsDialogView] = useState<SettingsDialogView | undefined>();

    const isSettingsDialogOpen = settingsDialogView !== undefined;

    function onCloseSettingsDialogClick() {
        setSettingsDialogView(undefined);
    }

    function onOpenSettingsDialogClick() {
        setSettingsDialogView(SettingsDialogView.SelectSetting);
    }

    return {
        isSettingsDialogOpen,
        settingsDialogView,
        setSettingsDialogView,
        onCloseSettingsDialogClick,
        onOpenSettingsDialogClick,
    };
}
