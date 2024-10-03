// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { Info, Warning } from '@iota/ui-icons';

export enum AlertType {
    Default = 'default',
    Warning = 'warning',
}

export enum AlertStyle {
    Default = 'default',
    Elevated = 'elevated',
}

interface AlertProps {
    type?: AlertType;
    title: string;
    supportingText?: string;
    style?: AlertStyle;
}

function mapAlertTypeToInfoBoxType(type: AlertType): InfoBoxType {
    switch (type) {
        case AlertType.Warning:
            return InfoBoxType.Warning;
        case AlertType.Default:
        default:
            return InfoBoxType.Default;
    }
}

function mapAlertStyleToInfoBoxStyle(style: AlertStyle): InfoBoxStyle {
    switch (style) {
        case AlertStyle.Elevated:
            return InfoBoxStyle.Elevated;
        case AlertStyle.Default:
        default:
            return InfoBoxStyle.Default;
    }
}

const MODE_TO_ICON = {
    default: <Info />,
    warning: <Warning />,
};

export function Alert({
    type = AlertType.Default,
    title,
    supportingText,
    style = AlertStyle.Elevated,
}: AlertProps) {
    return (
        <InfoBox
            type={mapAlertTypeToInfoBoxType(type)}
            title={title}
            icon={MODE_TO_ICON[type]}
            style={mapAlertStyleToInfoBoxStyle(style)}
            supportingText={supportingText}
        />
    );
}
