// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

interface MapFeatureProps {
    path: string | null;
}

export function MapFeature({ path }: MapFeatureProps): JSX.Element | null {
    if (!path) {
        return null;
    }

    return <path d={path} fill="white" strokeWidth={0.2} stroke="var(--steel-dark)" />;
}
