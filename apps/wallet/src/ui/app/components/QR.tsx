// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { QRCodeSVG } from 'qrcode.react';

export enum QRLevel {
    L = 'L',
    M = 'M',
    Q = 'Q',
    H = 'H',
}
interface QRProps {
    value: string;
    size: number;
    bgColor?: string;
    fgColor?: string;
    level?: QRLevel;
    marginSize?: number;
    title?: string;
}

export function QR({
    value,
    size,
    bgColor = '#ffffff',
    fgColor = '#000000',
    level = QRLevel.L,
    marginSize,
    title,
}: QRProps) {
    return (
        <QRCodeSVG
            value={value}
            size={size}
            bgColor={bgColor}
            fgColor={fgColor}
            level={level}
            marginSize={marginSize}
            title={title}
        />
    );
}
