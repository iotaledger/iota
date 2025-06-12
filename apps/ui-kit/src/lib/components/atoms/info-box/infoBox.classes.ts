// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InfoBoxType } from './infoBox.enums';

export const ICON_COLORS: Record<InfoBoxType, string> = {
    [InfoBoxType.Default]: 'bg-on-default text-default-surface',
    [InfoBoxType.Error]: 'bg-on-error text-error-surface',
    [InfoBoxType.Success]: 'bg-on-success text-success-surface',
    [InfoBoxType.Warning]: 'bg-on-warning text-warning-surface',
};

export const BACKGROUND_COLORS: Record<InfoBoxType, string> = {
    [InfoBoxType.Default]: 'bg-default-surface',
    [InfoBoxType.Error]: 'bg-error-surface',
    [InfoBoxType.Success]: 'bg-success-surface',
    [InfoBoxType.Warning]: 'bg-warning-surface',
};
