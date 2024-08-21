// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useParams } from 'react-router-dom';
import { PageLayout } from '~/components';
import { PageContent } from '~/pages/id-page/PageContent';

interface PageLayoutContainerProps {
    address: string;
}

function PageLayoutContainer({ address }: PageLayoutContainerProps): JSX.Element {
    return <PageLayout content={<PageContent address={address} />} />;
}

export function IdPage(): JSX.Element {
    const { id } = useParams();

    return <PageLayoutContainer address={id!} />;
}
