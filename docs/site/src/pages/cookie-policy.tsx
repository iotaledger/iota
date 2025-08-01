import { CookieLibrary } from '@boxfish-studio/react-cookie-manager';
import Layout from '@theme/Layout';
import './cookie-policy.css';
import React from 'react';

export default function CookiePolicy() {
  const configuration = {};

  return (
    <Layout
      title="Cookie Policy"
      description="Learn about our cookie policy and how we use cookies to enhance your experience."
    >
      <main className="mx-auto container px-6 py-16">
        <h1 className="text-5xl text-gray-900 mb-12 dark:text-white">
          Cookie Policy
        </h1>
        
        <div id="cookie-library-content">
          <CookieLibrary configuration={configuration} />
        </div>
      </main>
    </Layout>
  );
}
