// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use crate::cli::try_load_current_config;
use crate::config::{default_config_directory, default_config_filepath, default_data_directory};
use crate::{
    cli::{override_config, OverrideConfig},
    config::Config,
    error::NetworkRequesterError,
};
use clap::Args;
use nym_bin_common::output_format::OutputFormat;
use nym_client_core::client::key_manager::persistence::OnDiskKeys;
use nym_crypto::asymmetric::identity;
use nym_sphinx::addressing::clients::Recipient;
use serde::Serialize;
use std::fmt::Display;
use std::{fs, io};
use tap::TapFallible;

#[derive(Args, Clone)]
pub(crate) struct Init {
    /// Id of the nym-mixnet-client we want to create config for.
    #[clap(long)]
    id: String,

    /// Id of the gateway we are going to connect to.
    #[clap(long)]
    gateway: Option<identity::PublicKey>,

    /// Specifies whether the new gateway should be determined based by latency as opposed to being chosen
    /// uniformly.
    #[clap(long, conflicts_with = "gateway")]
    latency_based_selection: bool,

    /// Force register gateway. WARNING: this will overwrite any existing keys for the given id,
    /// potentially causing loss of access.
    #[clap(long)]
    force_register_gateway: bool,

    /// Comma separated list of rest endpoints of the nyxd validators
    #[clap(long, alias = "nymd_validators", value_delimiter = ',')]
    nyxd_urls: Option<Vec<url::Url>>,

    /// Comma separated list of rest endpoints of the API validators
    #[clap(long, alias = "api_validators", value_delimiter = ',')]
    // the alias here is included for backwards compatibility (1.1.4 and before)
    nym_apis: Option<Vec<url::Url>>,

    /// Set this client to work in a enabled credentials mode that would attempt to use gateway
    /// with bandwidth credential requirement.
    #[clap(long)]
    enabled_credentials_mode: Option<bool>,

    #[clap(short, long, default_value_t = OutputFormat::default())]
    output: OutputFormat,
}

impl From<Init> for OverrideConfig {
    fn from(init_config: Init) -> Self {
        OverrideConfig {
            nym_apis: init_config.nym_apis,
            fastmode: false,
            no_cover: false,

            nyxd_urls: init_config.nyxd_urls,
            enabled_credentials_mode: init_config.enabled_credentials_mode,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct InitResults {
    #[serde(flatten)]
    client_core: nym_client_core::init::InitResults,
    client_address: String,
}

impl InitResults {
    fn new(config: &Config, address: &Recipient) -> Self {
        Self {
            client_core: nym_client_core::init::InitResults::new(&config.base, address),
            client_address: address.to_string(),
        }
    }
}

impl Display for InitResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.client_core)?;
        write!(
            f,
            "Address of this network-requester: {}",
            self.client_address
        )
    }
}

fn init_paths(id: &str) -> io::Result<()> {
    fs::create_dir_all(default_data_directory(id))?;
    fs::create_dir_all(default_config_directory(id))
}

pub(crate) async fn execute(args: &Init) -> Result<(), NetworkRequesterError> {
    eprintln!("Initialising client...");

    let id = &args.id;

    let old_config = if default_config_filepath(id).exists() {
        eprintln!("Client \"{id}\" was already initialised before");
        // if the file exist, try to load it (with checking for errors)
        Some(try_load_current_config(&args.id)?)
    } else {
        init_paths(&args.id)?;
        None
    };

    // Usually you only register with the gateway on the first init, however you can force
    // re-registering if wanted.
    let user_wants_force_register = args.force_register_gateway;
    if user_wants_force_register {
        eprintln!("Instructed to force registering gateway. This might overwrite keys!");
    }

    // If the client was already initialized, don't generate new keys and don't re-register with
    // the gateway (because this would create a new shared key).
    // Unless the user really wants to.
    let register_gateway = old_config.is_none() || user_wants_force_register;

    // Attempt to use a user-provided gateway, if possible
    let user_chosen_gateway_id = args.gateway;

    // Load and potentially override config
    let mut config = override_config(Config::new(id), OverrideConfig::from(args.clone()));

    // Setup gateway by either registering a new one, or creating a new config from the selected
    // one but with keys kept, or reusing the gateway configuration.
    let key_store = OnDiskKeys::new(config.storage_paths.common_paths.keys.clone());
    let gateway = nym_client_core::init::setup_gateway_from_config::<_>(
        &key_store,
        register_gateway,
        user_chosen_gateway_id,
        &config.base,
        old_config.map(|cfg| cfg.base.client.gateway_endpoint),
        args.latency_based_selection,
    )
    .await
    .map_err(|source| {
        eprintln!("Failed to setup gateway\nError: {source}");
        NetworkRequesterError::FailedToSetupGateway { source }
    })?;

    config.base.set_gateway_endpoint(gateway);

    let config_save_location = config.default_location();
    config.save_to_default_location().tap_err(|_| {
        log::error!("Failed to save the config file");
    })?;
    eprintln!(
        "Saved configuration file to {}",
        config_save_location.display()
    );

    let address = nym_client_core::init::get_client_address_from_stored_ondisk_keys(
        &config.storage_paths.common_paths.keys,
        &config.base.client.gateway_endpoint,
    )?;

    eprintln!("Client configuration completed.\n");

    let init_results = InitResults::new(&config, &address);
    println!("{}", args.output.format(&init_results));

    Ok(())
}
