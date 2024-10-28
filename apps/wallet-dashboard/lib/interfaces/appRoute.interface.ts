// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface AppRoute {
    title: string;
    path: string;
    icon: (props: React.SVGProps<SVGSVGElement>) => React.JSX.Element;
}
