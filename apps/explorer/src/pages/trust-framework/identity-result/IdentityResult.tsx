// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { useParams } from 'react-router-dom';
import { PageLayout } from '~/components';
import { useDecodeDidFromUrl } from '~/hooks/useDecodeDidFromUrl';
import { Warning } from '@iota/apps-ui-icons';
import { IdentityContent } from './views/IdentityContent';

export function IdentityResult() {
    const { id: encodedDid } = useParams();
    const { data: decodedDid, isPending } = useDecodeDidFromUrl(encodedDid);

    if (isPending) {
        return <PageLayout loading loadingText="Decoding DID..." content={[]} />;
    }

    if (decodedDid == null) {
        return (
            <PageLayout
                content={
                    <InfoBox
                        title="Error parsing DID from URL"
                        supportingText={`A DID could not be parsed on the following URL ID: ${encodedDid}`}
                        icon={<Warning />}
                        type={InfoBoxType.Error}
                        style={InfoBoxStyle.Elevated}
                    />
                }
            />
        );
    }

    return <IdentityContent did={decodedDid} />;
}
