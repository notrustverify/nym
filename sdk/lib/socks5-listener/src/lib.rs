// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use crate::config::{config_filepath_from_root, Config};
use crate::persistence::MobileClientStorage;
use ::safer_ffi::prelude::*;
use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use log::{info, warn};
use nym_bin_common::logging::setup_logging;
use nym_client_core::client::key_manager::persistence::InMemEphemeralKeys;
use nym_client_core::client::key_manager::persistence::OnDiskKeys;
use nym_config_common::defaults::setup_env;
use nym_socks5_client_core::NymClient as Socks5NymClient;
use safer_ffi::char_p::char_p_boxed;
use std::marker::Send;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, Notify};

#[cfg(target_os = "android")]
pub mod android;
mod config;
mod persistence;

static SOCKS5_CONFIG_ID: &str = "mobile-socks5-test";

// hehe, this is so disgusting : )
lazy_static! {
    static ref CLIENT_SHUTDOWN_HANDLE: Mutex<Option<Arc<Notify>>> = Mutex::new(None);
    static ref RUNTIME: Runtime = Runtime::new().unwrap();
}
static ENV_SET: AtomicBool = AtomicBool::new(false);

async fn set_shutdown_handle(handle: Arc<Notify>) {
    let mut guard = CLIENT_SHUTDOWN_HANDLE.lock().await;
    if guard.is_some() {
        panic!("client wasn't properly stopped")
    }
    *guard = Some(handle)
}

async fn stop_and_reset_shutdown_handle() {
    let mut guard = CLIENT_SHUTDOWN_HANDLE.lock().await;
    if let Some(sh) = &*guard {
        sh.notify_waiters()
    } else {
        panic!("client wasn't properly started")
    }

    *guard = None
}

async fn is_shutdown_handle_set() -> bool {
    CLIENT_SHUTDOWN_HANDLE.lock().await.is_some()
}

fn set_default_env() {
    if !ENV_SET.swap(true, Ordering::SeqCst) {
        setup_env(None);
    }
}

// to be used with the on startup callback which returns the address
#[ffi_export]
pub fn rust_free_string(string: char_p::Box) {
    drop(string)
}

#[ffi_export]
pub fn initialise_logger() {
    setup_logging();
    info!("logger initialised");
}

#[derive_ReprC]
#[ffi_export]
#[repr(u8)]
#[derive(Eq, PartialEq, Debug)]
pub enum ClientState {
    Uninitialised,
    Connected,
    Disconnected,
}

#[ffi_export]
pub fn get_client_state() -> ClientState {
    // if the environment is not set, we never called start before
    // if the shutdown was never set, the client can't possibly be running
    // and similarly if it's set, it's most likely running
    if !ENV_SET.load(Ordering::Relaxed) {
        ClientState::Uninitialised
    } else if RUNTIME.block_on(is_shutdown_handle_set()) {
        ClientState::Connected
    } else {
        ClientState::Disconnected
    }
}

pub fn start_client<F, S>(
    storage_directory: Option<char_p::Ref<'_>>,
    service_provider: Option<char_p::Ref<'_>>,
    on_start_callback: F,
    on_shutdown_callback: S,
) where
    F: FnMut(String) + Send + 'static,
    S: FnMut() + Send + 'static,
{
    if get_client_state() == ClientState::Connected {
        warn!("could not start the client as it's already running");
        return;
    }

    let storage_dir = storage_directory.map(|s| s.to_string());
    let service_provider = service_provider.map(|s| s.to_string());
    RUNTIME.spawn(async move {
        _async_run_client(
            storage_dir,
            SOCKS5_CONFIG_ID.to_string(),
            service_provider,
            on_start_callback,
            on_shutdown_callback,
        )
        .await
    });
}

#[ffi_export]
pub fn stop_client() {
    if get_client_state() == ClientState::Disconnected {
        warn!("could not stop the client as it's not running    ");
        return;
    }

    RUNTIME.block_on(async move { stop_and_reset_shutdown_handle().await });
}

pub fn blocking_run_client<'cb, F, S>(
    storage_directory: Option<char_p::Ref<'_>>,
    service_provider: Option<char_p::Ref<'_>>,
    on_start_callback: F,
    on_shutdown_callback: S,
) where
    F: FnMut(String) + 'cb,
    S: FnMut() + 'cb,
{
    if get_client_state() == ClientState::Connected {
        warn!("could not start the client as it's already running");
        return;
    }

    let storage_dir = storage_directory.map(|s| s.to_string());
    let service_provider = service_provider.map(|s| s.to_string());
    RUNTIME
        .block_on(async move {
            _async_run_client(
                storage_dir,
                SOCKS5_CONFIG_ID.to_string(),
                service_provider,
                on_start_callback,
                on_shutdown_callback,
            )
            .await
        })
        .unwrap();
}

#[ffi_export]
pub fn reset_client_data(root_directory: char_p::Ref<'_>) {
    if get_client_state() == ClientState::Connected {
        return;
    }

    let root_dir = root_directory.to_string();
    _reset_client_data(root_dir)
}

#[ffi_export]
pub fn existing_service_provider(storage_directory: char_p::Ref<'_>) -> Option<char_p_boxed> {
    if let Ok(config) = Config::read_from_default_path(storage_directory.to_str(), SOCKS5_CONFIG_ID)
    {
        Some(config.core.socks5.provider_mix_address.try_into().unwrap())
    } else {
        None
    }
}

fn _reset_client_data(root_directory: String) {
    let client_storage_dir = PathBuf::new().join(root_directory).join(SOCKS5_CONFIG_ID);
    std::fs::remove_dir_all(client_storage_dir).expect("failed to clear client data")
}

async fn _async_run_client<F, S>(
    storage_dir: Option<String>,
    client_id: String,
    service_provider: Option<String>,
    mut on_start_callback: F,
    mut on_shutdown_callback: S,
) -> anyhow::Result<()>
where
    F: FnMut(String),
    S: FnMut(),
{
    set_default_env();
    let stop_handle = Arc::new(Notify::new());
    set_shutdown_handle(stop_handle.clone()).await;

    let config = load_or_generate_base_config(storage_dir, client_id, service_provider).await?;
    let storage = MobileClientStorage::new(&config);
    let socks5_client = Socks5NymClient::new(config.core, storage);

    eprintln!("starting the socks5 client");
    let mut started_client = socks5_client.start().await?;
    eprintln!("the client has started!");

    // invoke the callback since we've started!
    on_start_callback(started_client.address.to_string());

    // wait for notify to be set...
    stop_handle.notified().await;

    // and then do graceful shutdown of all tasks
    started_client.shutdown_handle.signal_shutdown().ok();
    started_client.shutdown_handle.wait_for_shutdown().await;

    // and the corresponding one for shutdown!
    on_shutdown_callback();

    Ok(())
}

// note: it does might not contain any gateway configuration and should not be persisted in that state!
async fn load_or_generate_base_config(
    storage_dir: Option<String>,
    client_id: String,
    service_provider: Option<String>,
) -> Result<Config> {
    let Some(storage_dir) = storage_dir else {
        eprintln!("no storage path specified");
        return setup_new_client_config(None, client_id, service_provider).await;
    };

    let expected_store_path = config_filepath_from_root(&storage_dir, &client_id);
    eprintln!(
        "attempting to load socks5 config from {}",
        expected_store_path.display()
    );

    // simulator workaround
    if let Ok(mut config) = Config::read_from_toml_file(expected_store_path) {
        eprintln!("loaded config");
        if let Some(storage_paths) = &mut config.storage_paths {
            if !storage_paths
                .common_paths
                .keys
                .public_identity_key_file
                .starts_with(&storage_dir)
            {
                eprintln!("... but it seems to have been made for different container - fixing it up... (ASSUMING DEFAULT PATHS)");
                storage_paths.change_root(storage_dir, &config.core.base.client.id);
            }
        }

        return Ok(config);
    };

    eprintln!("creating new config");
    setup_new_client_config(Some(storage_dir), client_id, service_provider).await
}

async fn setup_new_client_config(
    storage_dir: Option<String>,
    client_id: String,
    service_provider: Option<String>,
) -> Result<Config> {
    let service_provider = service_provider.ok_or(anyhow!(
        "service provider was not specified for fresh config"
    ))?;

    let mut new_config = Config::new(storage_dir.as_ref(), client_id, service_provider);

    if let Ok(raw_validators) = std::env::var(nym_config_common::defaults::var_names::NYM_API) {
        new_config
            .core
            .base
            .set_custom_nym_apis(nym_config_common::parse_urls(&raw_validators));
    }

    let gateway = if let Some(storage_paths) = &new_config.storage_paths {
        nym_client_core::init::setup_gateway_from_config::<_>(
            &OnDiskKeys::new(storage_paths.common_paths.keys.clone()),
            true,
            None,
            &new_config.core.base,
            None,
            false,
        )
        .await?
    } else {
        nym_client_core::init::setup_gateway_from_config::<_>(
            &InMemEphemeralKeys,
            true,
            None,
            &new_config.core.base,
            None,
            false,
        )
        .await?
    };

    new_config.core.base.set_gateway_endpoint(gateway);

    if let Some(storage_dir) = storage_dir {
        new_config.save_to_default_location(storage_dir)?;
    }

    Ok(new_config)
}

#[cfg(feature = "headers")] // c.f. the `Cargo.toml` section
pub fn generate_headers() -> ::std::io::Result<()> {
    ::safer_ffi::headers::builder()
        .to_file("socks5_c.h")?
        .generate()
}
