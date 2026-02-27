// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::net::SocketAddr;

use reqwest::Url;

use crate::client::Instance;

#[macro_export(local_inner_macros)]
macro_rules! ensure {
    ($cond:expr, $e:expr) => {
        if !($cond) {
            return Err($e);
        }
    };
}

pub type SettingsResult<T> = Result<T, SettingsError>;

#[derive(thiserror::Error, Debug)]
pub enum SettingsError {
    #[error("Failed to read settings file '{file:?}': {message}")]
    InvalidSettings { file: String, message: String },

    #[error("Failed to read token file '{file:?}': {message}")]
    InvalidTokenFile { file: String, message: String },

    #[error("Failed to read ssh public key file '{file:?}': {message}")]
    InvalidSshPublicKeyFile { file: String, message: String },

    #[error("Malformed repository url: {0:?}")]
    MalformedRepositoryUrl(Url),
}

pub type CloudProviderResult<T> = Result<T, CloudProviderError>;

#[derive(thiserror::Error, Debug)]
pub enum CloudProviderError {
    #[error("Failed to send server request: {0}")]
    Request(String),

    #[error("Unexpected response: {0}")]
    UnexpectedResponse(String),

    #[error("Received error status code ({0}): {1}")]
    FailureResponseCode(String, String),

    #[error("SSH key \"{0}\" not found")]
    SshKeyNotFound(String),

    #[error("Spot Instances cannot be stopped: Instance Id: {0}")]
    FailedToStopSpotInstance(String),
}

pub type SshResult<T> = Result<T, SshError>;

#[derive(thiserror::Error, Debug)]
pub enum SshError {
    #[error("Failed to load private key for {address}: {error}")]
    PrivateKeyError {
        address: SocketAddr,
        error: russh::keys::Error,
    },

    #[error("Failed to create ssh session with {address}: {error}")]
    SessionError {
        address: SocketAddr,
        error: russh::Error,
    },

    #[error("Failed to connect to instance {address}: {error}")]
    ConnectionError {
        address: SocketAddr,
        error: russh::Error,
    },

    #[error("Remote execution cmd '{command}' on {address} returned exit code ({code}): {message}")]
    NonZeroExitCode {
        address: SocketAddr,
        code: u32,
        message: String,
        command: String,
    },
}

pub type MonitorResult<T> = Result<T, MonitorError>;

#[derive(thiserror::Error, Debug)]
pub enum MonitorError {
    #[error(transparent)]
    Ssh(#[from] SshError),

    #[error("Failed to start Grafana: {0}")]
    Grafana(String),
}

pub type TestbedResult<T> = Result<T, TestbedError>;

#[derive(thiserror::Error, Debug)]
pub enum TestbedError {
    #[error(transparent)]
    Settings(#[from] SettingsError),

    #[error(transparent)]
    CloudProvider(#[from] CloudProviderError),

    #[error(transparent)]
    Ssh(#[from] SshError),

    #[error("Failed to execute command '{1}' over ssh on instance {0}: {2}")]
    SshCommandFailed(Instance, String, String),

    #[error("Not enough instances: missing {0} instances")]
    InsufficientCapacity(usize),

    #[error("Metrics instance is missing")]
    MetricsServerMissing(),

    #[error("Not enough dedicated client instances: missing {0} instances")]
    InsufficientDedicatedClientCapacity(usize),

    #[error(transparent)]
    Monitor(#[from] MonitorError),

    #[error(transparent)]
    BuildCacheError(#[from] iota_build_cache_server::client::BuildCacheError),
}
