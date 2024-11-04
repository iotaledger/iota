import React, { useState, useMemo } from 'react';
import {
  IotaClientProvider,
  WalletProvider,
} from '@iota/dapp-kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';

// Define props interface
interface ChallengeVerifierProps {
  expectedObjectType: string; // Prop for the expected Object Type
}

// Define network configurations
const NETWORKS = {
  devnet: { url: getFullnodeUrl('devnet') },
  alphanet: { url: 'https://api.iota-rebased-alphanet.iota.cafe:443' },
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
      const client = new IotaClient({ url: NETWORKS.alphanet.url });
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
    <div>
      <input
        type="text"
        value={inputText}
        onChange={(e) => setInputText(e.target.value)}
        placeholder="Enter Flag Object Id"
      />
      <button onClick={handleSubmit} disabled={loading}>
        {loading ? 'Loading...' : 'Submit'}
      </button>

      {error && <p style={{ color: 'red' }}>{error}</p>}

      {coins && (
        <div>
          <h2>Result</h2>
          <pre>{JSON.stringify(coins, null, 2)}</pre>
        </div>
      )}
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
