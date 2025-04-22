// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface ProtectedRoute {
    title: string;
    path: string;
    icon: (props: React.SVGProps<SVGSVGElement>) => React.JSX.Element;
    id: string;
}

export interface PublicRoute extends Pick<ProtectedRoute, 'path'> {}
