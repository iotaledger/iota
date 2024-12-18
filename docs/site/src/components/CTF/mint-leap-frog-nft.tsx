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
import { handleMintLeapFrogSubmit } from "../../utils/ctf-utils"

const NETWORKS = {
  testnet: { url: getFullnodeUrl('testnet') },
};

const MintLeapFrogNFT: React.FC = () => {
  const [nft, setNFT] = useState({
    name:'',
    description:'',
    url:'',
  });
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
   await handleMintLeapFrogSubmit({
      nft,
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
    <div>
      <div className="flex items-center">
      <input
        type="text"
        value={nft.name}
        onChange={(e) => setNFT((prevState) => ({
          ...prevState,
          name:e.target.value
        }))}
        placeholder="Enter name"
        className="input-field mr-2"
      />
      <input
        type="text"
        value={nft.description}
        onChange={(e) => setNFT((prevState) => ({
          ...prevState,
          description:e.target.value
        }))}
        placeholder="Enter description"
        className="input-field mr-2"
      />
      <input
        type="text"
        value={nft.url}
        onChange={(e) => setNFT((prevState) => ({
          ...prevState,
          url:e.target.value
        }))}
        placeholder="Enter url"
        className="input-field mr-2"
      />
      <button
        onClick={handleSubmit}
        className={`${clsx('button', { 'button-disabled': loading })} p-3 min-w-28`}
        disabled={loading|| coins==="Congratulations! You have successfully completed this level!" }
      >
        {loading ? 'Loading...' : 'Submit'}
      </button>
      </div>
      <div className="flex items-center">
      {coins && !loading && <pre className="mt-2 mb-0 p-3">{coins}</pre>}
      </div>
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

const withProviders = (Component: React.FC) => {
  return () => {
    if (typeof window === 'undefined') {
      return null;
    }

    const queryClient = useMemo(() => new QueryClient(), []);

    return (
      <QueryClientProvider client={queryClient}>
        <IotaClientProvider networks={NETWORKS}>
          <WalletProvider>
            <Component/>
          </WalletProvider>
        </IotaClientProvider>
      </QueryClientProvider>
    );
  };
};

export default withProviders(MintLeapFrogNFT);