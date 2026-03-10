// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import {
    RadioButton,
    Table,
    TableBody,
    TableCellBase,
    TableCellText,
    TableHeader,
    TableHeaderCell,
    TableRow,
} from '@iota/apps-ui-kit';
import { useEffect, useState } from 'react';

import { getAmplitudeConsentStatus } from '@/lib/utils/analytics/amplitude';

import { CookiePolicyContentProps, DescribedCookie } from './types';

export function CookiePolicyContent({
    necessaryCookies,
    additionalCookies,
    onAccept,
    onReject,
}: CookiePolicyContentProps): React.ReactElement {
    const [consentStatus, setConsentStatus] = useState<'pending' | 'accepted' | 'declined'>(
        'pending',
    );

    useEffect(() => {
        const status = getAmplitudeConsentStatus();
        setConsentStatus(status);
    }, []);

    async function handleAccept() {
        await onAccept();
        setConsentStatus('accepted');
    }

    async function handleDecline() {
        await onReject();
        setConsentStatus('declined');
    }

    return (
        <div className="flex flex-col gap-md mb-lg min-h-screen">
            <h1 className="text-names-neutral-92 text-headline-lg mb-md">Cookie Policy</h1>
            <CookiePolicyContentSection>
                <CookiePolicyContentTitle>This website uses cookies</CookiePolicyContentTitle>
                <CookiePolicyContentDescription>
                    <>
                        <p>
                            We are using Cookies on our websites. We may use Cookies to identify the
                            browser you are using so that our website displays properly. We also use
                            cookies in various places on our website in order to document your visit
                            to our website and allow for a more efficient website design.
                        </p>
                        <p>
                            You may reject the setting of Cookies by adjusting the relevant settings
                            listed below. You can also disable Cookie installation via your browser
                            settings. You also have the option of deleting Cookies from your
                            computer's hard disk at any time.
                        </p>
                    </>
                </CookiePolicyContentDescription>
            </CookiePolicyContentSection>
            {necessaryCookies && necessaryCookies.length > 0 && (
                <CookiePolicyContentSection>
                    <CookiePolicyContentTitle>Necessary Cookies</CookiePolicyContentTitle>
                    <CookiePolicyContentDescription>
                        <p>
                            Necessary cookies help make a website usable by enabling basic functions
                            like page navigation and access to secure areas of the website. The
                            website cannot function properly without these cookies.
                        </p>
                    </CookiePolicyContentDescription>
                    <CookiesTable cookies={necessaryCookies} />
                </CookiePolicyContentSection>
            )}
            {additionalCookies && additionalCookies.length > 0 && (
                <CookiePolicyContentSection>
                    <CookiePolicyContentTitle>Additional Cookies</CookiePolicyContentTitle>
                    <CookiePolicyContentDescription>
                        <p>
                            Additional cookies help us improve our website by collecting and
                            reporting information on how you use it. These cookies may be set by us
                            or by third party providers whose services we have added to our pages.
                        </p>
                    </CookiePolicyContentDescription>
                    <CookiesTable cookies={additionalCookies} />
                </CookiePolicyContentSection>
            )}
            <CookiePolicyContentSection>
                <CookiePolicyContentTitle>Cookie Preferences</CookiePolicyContentTitle>
                <CookiePolicyContentDescription>
                    <p>
                        You can choose to accept or decline additional cookies. Necessary cookies
                        cannot be declined as they are essential for the website to function.
                    </p>
                </CookiePolicyContentDescription>
                <div className="mt-md flex flex-col gap-md">
                    <RadioButton
                        name="cookie-consent"
                        label="Accept all cookies"
                        body="Accept both necessary and additional cookies to help us improve our website."
                        isChecked={consentStatus === 'accepted'}
                        onChange={handleAccept}
                    />
                    <RadioButton
                        name="cookie-consent"
                        label="Reject all cookies"
                        body="Only accept necessary cookies required for the website to function."
                        isChecked={consentStatus === 'declined'}
                        onChange={handleDecline}
                    />
                </div>
            </CookiePolicyContentSection>
        </div>
    );
}

function CookiePolicyContentSection({
    children,
}: {
    children: React.ReactNode;
}): React.JSX.Element {
    return <div className="mb-lg gap-md">{children}</div>;
}

function CookiePolicyContentTitle({ children }: { children: React.ReactNode }): React.JSX.Element {
    return (
        <h2 className="text-names-neutral-92 dark:text-iota-primary-90 text-title-lg mb-md">
            {children}
        </h2>
    );
}

function CookiePolicyContentDescription({
    children,
}: {
    children: React.ReactNode;
}): React.JSX.Element {
    return (
        <div className="text-names-neutral-92 text-body-md [&_p:not(:last-child)]:mb-sm">
            {children}
        </div>
    );
}

function CookiesTable({ cookies }: { cookies: DescribedCookie[] }): React.JSX.Element {
    const HEADERS = [
        { label: 'Name', columnKey: 1 },
        { label: 'Provider', columnKey: 2 },
        { label: 'Category', columnKey: 3 },
        { label: 'Purpose', columnKey: 4 },
        { label: 'Expiration', columnKey: 5 },
    ];

    return (
        <div className="mt-md">
            <Table rowIndexes={cookies.map((_, index) => index)}>
                <TableHeader>
                    <TableRow>
                        {HEADERS.map((header) => (
                            <TableHeaderCell key={header.columnKey} {...header} />
                        ))}
                    </TableRow>
                </TableHeader>
                <TableBody>
                    {cookies.map((cookie, rowIndex) => (
                        <TableRow key={rowIndex}>
                            <TableCellBase>
                                <TableCellText>{cookie.name}</TableCellText>
                            </TableCellBase>
                            <TableCellBase>
                                <TableCellText>{cookie.provider || '-'}</TableCellText>
                            </TableCellBase>
                            <TableCellBase>
                                <TableCellText>{cookie.category || '-'}</TableCellText>
                            </TableCellBase>
                            <TableCellBase>
                                <TableCellText>{cookie.purpose || '-'}</TableCellText>
                            </TableCellBase>
                            <TableCellBase>
                                <TableCellText>{cookie.expiration || '-'}</TableCellText>
                            </TableCellBase>
                        </TableRow>
                    ))}
                </TableBody>
            </Table>
        </div>
    );
}
