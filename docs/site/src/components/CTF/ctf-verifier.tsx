import React, { useState, useMemo } from 'react';
import {
  IotaClientProvider,
  useSignAndExecuteTransaction,
  WalletProvider,
} from '@iota/dapp-kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { getFullnodeUrl } from '@iota/iota-sdk/client';
import clsx from 'clsx';
import { useConnectWallet, useWallets } from '@iota/dapp-kit';
import Popup from './popup';
import { handleChallengeSubmit } from "../../utils/ctf-utils"

interface ChallengeVerifierProps {
  expectedObjectType: string;
  nftName: string;
  challengeNumber: string
}

const NETWORKS = {
  testnet: { url: getFullnodeUrl('testnet') },
};

const ChallengeVerifier: React.FC<ChallengeVerifierProps> = ({
  expectedObjectType,
  nftName,
  challengeNumber,
}) => {
  const [inputText, setInputText] = useState('');
  const [coins, setCoins] = useState<string | null>(null);
  const [showPopup, setShowPopup] = useState<boolean>(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<{
    status: 'success' | 'error';
    description: string;
    title: string;
  }>({
    status: 'success',
    description: '',
    title: '',
  });

  const wallets = useWallets();
  const { mutate } = useConnectWallet();
  const { mutate: signAndExecuteTransaction } = useSignAndExecuteTransaction();

  const handleSubmit = async () => {
   await handleChallengeSubmit({
      inputText,
      expectedObjectType,
      nftName,
      challengeNumber,
      wallets,
      mutate,
      signAndExecuteTransaction,
      setLoading,
      setCoins,
      setError,
      setShowPopup,
    });
  };

  return (
    <div className="flex items-center">
      <input
        type="text"
        value={inputText}
        onChange={(e) => setInputText(e.target.value)}
        placeholder="Enter Flag Object Id"
        className="input-field mr-2"
      />
      <button
        onClick={handleSubmit}
        className={`${clsx('button', { 'button-disabled': loading })} p-3 min-w-28`}
        disabled={loading|| coins==="Congratulations! You have successfully completed this level!" }
      >
        {loading ? 'Loading...' : 'Submit'}
      </button>

      {error.status === 'error' && <p className="text-red-500 text-center mb-0 ml-2">{error.description}</p>}

      {coins && !loading && <pre className="ml-4 mb-0 p-3">{coins}</pre>}

      {showPopup && (
        <Popup
          status={error.status}
          description={error.description}
          title={error.title}
          setShowPopup={setShowPopup}
          showPopup={showPopup}
        />
      )}
    </div>
  );
};

const withProviders = (Component: React.FC<ChallengeVerifierProps>) => {
  return ({ expectedObjectType }: ChallengeVerifierProps) => {
    if (typeof window === 'undefined') {
      return null;
    }

    const queryClient = useMemo(() => new QueryClient(), []);

    return (
      <QueryClientProvider client={queryClient}>
        <IotaClientProvider networks={NETWORKS}>
          <WalletProvider>
            <Component
              expectedObjectType={expectedObjectType}
              challengeNumber="1"
              nftName="Checkin"
            />
          </WalletProvider>
        </IotaClientProvider>
      </QueryClientProvider>
    );
  };
};

export default withProviders(ChallengeVerifier);