// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CookiePolicy } from '~/components/disclaimer/CookiePolicy';
import { PageLayout } from '~/components';

export function CookiePolicyPage(): JSX.Element {
    return <PageLayout content={<CookiePolicy />} />;
}
