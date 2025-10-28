import { CookieManager, type SKCMConfiguration } from '@boxfish-studio/react-cookie-manager';

const COOKIES_KEY = 'AMP_COOKIES_ACCEPTED';

export function CookieDisclaimer() {
    const configuration: SKCMConfiguration = {
        disclaimer: {
            title: undefined,
            body: 'We use cookies and analytics tools to help us improve your experience. ',
            policyText: 'Read our Cookie Policy',
            policyUrl: 'https://www.iota.org/cookie-policy',
        },
        services: {
            customNecessaryCookies: [
                {
                    name: COOKIES_KEY,
                    purpose:
                        'Flag indicating that Amplitude analytics cookies may be created after consent',
                    expiry: '1 year',
                    type: 'http',
                    showDisclaimerIfMissing: true,
                },
            ],
        },
        onAcceptCookies: () => {
            document.cookie = `${COOKIES_KEY}=true; max-age=31536000`;
        },
        onDeclineCookies: () => {
            document.cookie = `${COOKIES_KEY}=false; max-age=31536000`;
        },
    };
    return <CookieManager configuration={configuration} />;
}
