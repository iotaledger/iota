// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { useNavigate } from 'react-router-dom';
import ImportAWallet from '_assets/images/onboarding/import-a-wallet.png';
import ImportAWalletDark from '_assets/images/onboarding/import-a-wallet-darkmode.png';
import { Card, CardType, CardBody, CardAction, CardActionType } from '@iota/apps-ui-kit';
import { AccountsFormType, PageTemplate, useSourceFlow } from '_components';
import { useAppSelector, useAccounts } from '_hooks';
import { ExtensionViewType } from '../../redux/slices/app/appType';
import { ImportPass, Key, Passkey, Firefly } from '@iota/apps-ui-icons';
import { openInNewTab } from '_src/shared/utils';
import { type ActionCardItem, OnboardingCardIcon } from './AddAccountPage';
import { Theme, useTheme } from '@iota/core';
import clsx from 'clsx';
import { isFirstAccount } from '../../helpers';
import { ACCOUNT_FORM_TYPE_TO_AMPLI } from '_src/shared/analytics';

export function ImportExistingWallet() {
    const { theme } = useTheme();
    const navigate = useNavigate();
    const isPopupOrSidePanel = useAppSelector(
        (state) =>
            state.app.extensionViewType === ExtensionViewType.Popup ||
            state.app.extensionViewType === ExtensionViewType.SidePanel,
    );
    const { sourceFlowRef } = useSourceFlow();
    const sourceFlow = sourceFlowRef.current;
    const { data: accounts } = useAccounts();

    const profileOptions = [
        {
            title: 'Mnemonic',
            icon: ImportPass,
            subtitle: 'Recovery Phrase (12/24 words)',
            actionType: AccountsFormType.ImportMnemonic,
        },
        {
            title: 'Private Key',
            icon: Key,
            subtitle: '64-characters (letters and numbers)',
            actionType: AccountsFormType.ImportPrivateKey,
        },
        {
            title: 'Passkey',
            icon: Passkey,
            subtitle: 'Use a password manager',
            actionType: AccountsFormType.ImportPasskey,
        },
    ] as const satisfies ActionCardItem[];

    const legacyOptions = [
        {
            title: 'Firefly Seed Recovery',
            icon: Firefly,
            actionType: AccountsFormType.ImportSeed,
        },
    ] as const satisfies ActionCardItem[];

    const handleCardAction = async (
        actionType: (typeof profileOptions | typeof legacyOptions)[number]['actionType'],
    ) => {
        const ampliData = ACCOUNT_FORM_TYPE_TO_AMPLI[actionType];

        if (ampliData) {
            ampli.clickedCreateNewAccount({
                accountType: ampliData.accountType,
                accountOrigin: ampliData.accountOrigin,
                isFirstAccount: isFirstAccount(accounts),
                sourceFlow,
            });
        }

        switch (actionType) {
            case AccountsFormType.ImportMnemonic:
                navigate('/accounts/import-passphrase');
                break;
            case AccountsFormType.ImportPrivateKey:
                navigate('/accounts/import-private-key');
                break;
            case AccountsFormType.ImportPasskey:
                const url = '/accounts/import-passkey';
                if (isPopupOrSidePanel) {
                    openInNewTab(`${url}?sourceFlow=${sourceFlow}`);
                    window.close();
                } else {
                    navigate(url);
                }
                break;
            case AccountsFormType.ImportSeed:
                navigate('/accounts/import-seed');
                break;
            default:
                throw new Error('Unsupported action type');
        }
    };

    const hasManyProfileOptions = profileOptions.length >= 3;

    return (
        <PageTemplate
            title="Import a wallet"
            isTitleCentered
            onClose={() => navigate('/')}
            showBackButton
            onBack={() => navigate('/accounts/add-account')}
        >
            <div className="flex h-full w-full flex-col">
                <div className="flex w-full flex-1 flex-col justify-center gap-4 py-md--rs text-center">
                    <img
                        src={theme === Theme.Dark ? ImportAWalletDark : ImportAWallet}
                        alt="Import a wallet"
                        height={140}
                        width="auto"
                        className="mx-auto mb-2 h-[140px] w-auto object-cover"
                    />
                    <div className="flex flex-col items-center gap-xxs">
                        <h2 className="font-alliance-no2 text-[20px] font-medium leading-[120%] tracking-[-0.4px] text-iota-neutral-10 dark:text-iota-neutral-92">
                            Choose your access method.
                        </h2>

                        <p className="text-body-lg font-normal text-iota-neutral-10 dark:text-iota-neutral-92">
                            Import or restore existing wallet
                        </p>
                    </div>
                </div>

                <div
                    className={clsx(
                        'flex flex-col gap-y-xs text-start',
                        hasManyProfileOptions ? 'py-md--rs' : 'pt-md--rs',
                    )}
                >
                    {profileOptions.map((card) => (
                        <Card
                            key={card.title}
                            type={CardType.Filled}
                            isHoverable
                            onClick={() => handleCardAction(card.actionType)}
                            testId={card.actionType}
                        >
                            <OnboardingCardIcon Icon={card.icon} />
                            <CardBody title={card.title} subtitle={card.subtitle} />
                            <CardAction type={CardActionType.Link} />
                        </Card>
                    ))}

                    <span className="pt-xxs text-label-lg capitalize leading-5 tracking-[-0.1px] text-iota-neutral-40 dark:text-iota-neutral-60">
                        Legacy recovery
                    </span>

                    {legacyOptions.map((card) => (
                        <Card
                            key={card.title}
                            type={CardType.Filled}
                            isHoverable
                            onClick={() => handleCardAction(card.actionType)}
                        >
                            <OnboardingCardIcon Icon={card.icon} isLegacy />
                            <CardBody title={card.title} />
                            <CardAction type={CardActionType.Link} />
                        </Card>
                    ))}
                </div>
            </div>
        </PageTemplate>
    );
}
