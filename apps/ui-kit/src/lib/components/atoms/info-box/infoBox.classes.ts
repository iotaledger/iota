// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InfoBoxType } from './infoBox.enums';

export const ICON_COLORS: Record<InfoBoxType, string> = {
    [InfoBoxType.Default]: 'bg-default-surface text-on-default',
    [InfoBoxType.Error]: 'bg-error-surface text-on-error',
    [InfoBoxType.Success]: 'bg-success-surface text-on-success',
    [InfoBoxType.Warning]: 'bg-warning-surface text-on-warning',
};

export const BACKGROUND_COLORS: Record<InfoBoxType, string> = {
    [InfoBoxType.Default]: 'bg-default-surface',
    [InfoBoxType.Error]: 'bg-error-surface',
    [InfoBoxType.Success]: 'bg-success-surface',
    [InfoBoxType.Warning]: 'bg-warning-surface',
};
