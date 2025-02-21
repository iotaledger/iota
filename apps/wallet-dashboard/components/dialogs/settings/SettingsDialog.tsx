// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Dialog } from '@iota/apps-ui-kit';
import { SettingsDialogView } from './enums';
import { SettingsListView, NetworkSelectorView } from './views';

interface SettingsDialogProps {
    isOpen: boolean;
    handleClose: () => void;
    view: SettingsDialogView | undefined;
    setView: (view: SettingsDialogView) => void;
}

export function SettingsDialog({
    isOpen,
    handleClose,
    view,
    setView,
}: SettingsDialogProps): JSX.Element {
    function onBack(): void {
        setView(SettingsDialogView.SelectSetting);
    }

    return (
        <Dialog open={isOpen} onOpenChange={() => handleClose()}>
            {view === SettingsDialogView.SelectSetting && (
                <SettingsListView handleClose={handleClose} setView={setView} />
            )}
            {view === SettingsDialogView.NetworkSettings && (
                <NetworkSelectorView handleClose={handleClose} onBack={onBack} />
            )}
        </Dialog>
    );
}
