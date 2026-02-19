// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { useNavigate, useSearchParams } from 'react-router-dom';
import SecureYourWallet from '_assets/images/onboarding/secure-your-wallet.png';
import SecureYourWalletDark from '_assets/images/onboarding/secure-your-wallet-darkmode.png';
import { Card, CardType, CardBody, CardAction, CardActionType } from '@iota/apps-ui-kit';
import { AccountsFormType, useAccountsFormContext, PageTemplate } from '_components';
import { useAppSelector, useCreateAccountsMutation } from '_hooks';
import { ExtensionViewType } from '../../redux/slices/app/appType';
import { ImportPass, Passkey } from '@iota/apps-ui-icons';
import { openInNewTab } from '_src/shared/utils';
import { type ActionCardItem, OnboardingCardIcon } from './AddAccountPage';
import { Feature, Theme, useFeatureEnabledByNetwork, useTheme } from '@iota/core';

export function CreateNewWallet() {
    const { theme } = useTheme();
    const navigate = useNavigate();
    const [, setAccountsFormValues] = useAccountsFormContext();
    const network = useAppSelector(({ app }) => app.network);
    const isPopupOrSidePanel = useAppSelector((state) =>
        [ExtensionViewType.Popup, ExtensionViewType.SidePanel].includes(
            state.app.extensionViewType,
        ),
    );
    const createAccountsMutation = useCreateAccountsMutation();
    const [searchParams] = useSearchParams();
    const sourceFlow = searchParams.get('sourceFlow') || 'Unknown';
    const isPasskeysEnabled = useFeatureEnabledByNetwork(Feature.WalletPasskeys, network);

    const profileOptions = [
        {
            title: 'Mnemonic',
            icon: ImportPass,
            subtitle: 'Recovery Phrase (12/24 words)',
            actionType: AccountsFormType.NewMnemonic,
        },
        ...(isPasskeysEnabled
            ? [
                  {
                      title: 'Passkey',
                      icon: Passkey,
                      subtitle: 'Use a password manager',
                      actionType: AccountsFormType.Passkey,
                  },
              ]
            : []),
    ] as const satisfies ActionCardItem[];

    const handleCardAction = async (actionType: (typeof profileOptions)[number]['actionType']) => {
        switch (actionType) {
            case AccountsFormType.NewMnemonic:
                setAccountsFormValues({ type: AccountsFormType.NewMnemonic });
                ampli.accountCreationStarted({ sourceFlow });
                navigate(
                    `/accounts/protect-account?accountsFormType=${AccountsFormType.NewMnemonic}`,
                );
                break;
            case AccountsFormType.Passkey:
                ampli.clickedCreatePasskey({ sourceFlow });
                const url = '/accounts/passkey-account';
                if (isPopupOrSidePanel) {
                    openInNewTab(url);
                    window.close();
                } else {
                    navigate(url);
                }
                break;
            default:
                throw new Error('Unsupported action type');
        }
    };

    return (
        <PageTemplate
            title="Create a new wallet"
            isTitleCentered
            onClose={() => navigate('/')}
            showBackButton
            onBack={() => navigate('/accounts/add-account')}
        >
            <div className="flex h-full w-full flex-col">
                <div className="flex w-full flex-1 flex-col justify-center py-md--rs text-center">
                    <div className="flex flex-col items-center gap-y-4">
                        <img
                            src={theme === Theme.Dark ? SecureYourWalletDark : SecureYourWallet}
                            alt="Secure your wallet"
                            height={178}
                            className="mx-auto aspect-[4/3] h-[178px] w-auto object-cover"
                        />
                        <div className="flex flex-1 flex-col items-center gap-xxs">
                            <h2 className="text-headline-sm font-medium leading-[120%] tracking-[-0.4px] text-iota-neutral-10 dark:text-iota-neutral-92">
                                Secure your wallet
                            </h2>
                        </div>
                    </div>
                </div>

                <div className="flex flex-col gap-y-xs pt-md--rs text-start">
                    {profileOptions.map((card) => (
                        <Card
                            key={card.title}
                            type={CardType.Filled}
                            isDisabled={createAccountsMutation.isPending}
                            isHoverable
                            onClick={() => handleCardAction(card.actionType)}
                            testId={card.actionType}
                        >
                            <OnboardingCardIcon Icon={card.icon} />
                            <CardBody title={card.title} subtitle={card.subtitle} />
                            <CardAction type={CardActionType.Link} />
                        </Card>
                    ))}
                </div>
            </div>
        </PageTemplate>
    );
}
