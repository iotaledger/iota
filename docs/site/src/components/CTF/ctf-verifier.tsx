import React, { useState, useMemo } from 'react';
import {
  IotaClientProvider,
  WalletProvider,
} from '@iota/dapp-kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import clsx from 'clsx';

// Define props interface
interface ChallengeVerifierProps {
  expectedObjectType: string; // Prop for the expected Object Type
}

// Define network configurations
const NETWORKS = {
  testnet: { url: getFullnodeUrl('testnet') },
};

// Main ChallengeVerifier component
const ChallengeVerifier: React.FC<ChallengeVerifierProps> = ({ expectedObjectType }) => {
  const [inputText, setInputText] = useState('');
  const [coins, setCoins] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async () => {
    setLoading(true);
    setError('');
    setCoins(null);

    try {
      const client = new IotaClient({ url: NETWORKS.testnet.url });
      const result = await client.getObject({ id: inputText, options: { showType: true } });

      const message = result.data.type === expectedObjectType
        ? 'Congratulations! You have successfully completed this level!'
        : 'Invalid Flag Object Id. Please try again.';

      setCoins(message);
    } catch (err: any) {
      setError(err.message || 'An error occurred. Please try again.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className='bg-[#0000001a] dark:bg-[#1e1e1e] p-5 rounded-lg'>
      <h3>Claim your NFT reward</h3>
      <label >Flag Id <span className="red">*</span></label>
      <div className='flex flex-col flex-wrap items-start mt-1'>
        <input
          type="text"
          value={inputText}
          onChange={(e) => setInputText(e.target.value)}
          placeholder="Enter Flag Object Id"
          className="input-field"
        />
        {<p className={`text-red-500 mb-0 mt-1 text-sm ${error ? 'visible' : 'invisible'}`}>{error}</p>}
        <button 
          onClick={handleSubmit} 
          className={`${clsx("button", { "button-disabled": inputText=='' || loading })} min-w-28 mt-4`}
          disabled={inputText=='' || loading}
        >
          {loading ? 'Loading...' : 'Submit Your Challenge'}
        </button>
        {coins && <p className='mb-0 py-3 px-2 bg-[#353535] rounded-md'>{coins}</p>}
      </div>
    </div>
  );
};

// Higher-order function to provide necessary context
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
            <Component expectedObjectType={expectedObjectType} />
          </WalletProvider>
        </IotaClientProvider>
      </QueryClientProvider>
    );
  };
};

// Export the component wrapped in providers
export default withProviders(ChallengeVerifier);