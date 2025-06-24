import { IotaLogoWeb } from '@iota/apps-ui-icons';
import { Divider } from '@iota/apps-ui-kit';
import { Link } from '../link';

interface ExternalLink {
    text: string;
    url: string;
}

const EXTERNAL_LINKS: ExternalLink[] = [
    {
        text: 'Discord',
        url: 'https://discord.iota.org/',
    },
    {
        text: 'LinkedIn',
        url: 'https://www.linkedin.com/company/iotafoundation/',
    },
    {
        text: 'Twitter',
        url: 'https://twitter.com/iota',
    },
    {
        text: 'GitHub',
        url: 'https://www.github.com/iotaledger/',
    },
    {
        text: 'Youtube',
        url: 'https://www.youtube.com/c/iotafoundation',
    },
];

const USE_CONDITIONS_LINKS: ExternalLink[] = [
    {
        text: 'Terms & Conditions',
        url: 'https://www.iota.org/terms-of-use',
    },
    {
        text: 'Privacy Policy',
        url: 'https://www.iota.org/privacy-policy',
    },
];

export function Footer() {
    return (
        <footer className="w-full  dark:bg-neutral-10 bg-neutral-92 pt-lg pb-2xl">
            <div className="container flex flex-col justify-center gap-y-lg md:gap-md--rs">
                <div className="flex flex-col md:flex-row justify-between items-center gap-y-lg">
                    <IotaLogoWeb className="w-36 h-9 dark:text-neutral-92 text-neutral-10" />
                    <div className="flex flex-row gap-lg items-center">
                        {EXTERNAL_LINKS.map(({ url, text }) => (
                            <Link key={text} href={url} isSecondary>
                                {text}
                            </Link>
                        ))}
                    </div>
                </div>

                <Divider />

                <div className="flex flex-col-reverse md:flex-row justify-between items-center gap-y-lg">
                    <span className="text-neutral-40 dark:text-neutral-60 text-body-md tracking-normal">
                        © {new Date().getFullYear()} IOTA Foundation. All Rights Reserved.
                    </span>

                    <div className="flex flex-row gap-lg items-center">
                        {USE_CONDITIONS_LINKS.map(({ url, text }) => (
                            <Link key={text} href={url} isSecondary>
                                {text}
                            </Link>
                        ))}
                    </div>
                </div>

                <span className="w-full text-center text-neutral-40 dark:text-neutral-60 text-label-md">
                    {COMMIT_REV}
                </span>
            </div>
        </footer>
    );
}
