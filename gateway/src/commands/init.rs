// Copyright 2020 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use crate::{
    commands::{override_config, OverrideConfig},
    config::{persistence::pathfinder::GatewayPathfinder, Config},
};
use clap::Args;
use config::NymConfig;
use crypto::asymmetric::{encryption, identity};

#[derive(Args, Clone)]
pub struct Init {
    /// Id of the gateway we want to create config for
    #[clap(long)]
    id: String,

    /// The custom host on which the gateway will be running for receiving sphinx packets
    #[clap(long)]
    host: String,

    /// The wallet address you will use to bond this gateway, e.g. nymt1z9egw0knv47nmur0p8vk4rcx59h9gg4zuxrrr9
    #[clap(long)]
    wallet_address: String,

    /// The port on which the gateway will be listening for sphinx packets
    #[clap(long)]
    mix_port: Option<u16>,

    /// The port on which the gateway will be listening for clients gateway-requests
    #[clap(long)]
    clients_port: Option<u16>,

    /// The host that will be reported to the directory server
    #[clap(long)]
    announce_host: Option<String>,

    /// Path to sqlite database containing all gateway persistent data
    #[clap(long)]
    datastore: Option<String>,

    /// Comma separated list of endpoints of the validators APIs
    #[clap(long)]
    validator_apis: Option<String>,

    /// Comma separated list of endpoints of the validator
    #[clap(long)]
    validators: Option<String>,

    /// Cosmos wallet mnemonic needed for double spending protection
    #[clap(long)]
    mnemonic: Option<String>,

    /// Set this gateway to work only with coconut credentials; that would disallow clients to
    /// bypass bandwidth credential requirement
    #[cfg(feature = "coconut")]
    #[clap(long)]
    only_coconut_credentials: bool,

    /// Enable/disable gateway anonymized statistics that get sent to a statistics aggregator server
    #[clap(long)]
    enabled_statistics: Option<bool>,

    /// URL where a statistics aggregator is running. The default value is a Nym aggregator server
    #[clap(long)]
    statistics_service_url: Option<String>,
}

impl From<Init> for OverrideConfig {
    fn from(init_config: Init) -> Self {
        OverrideConfig {
            host: Some(init_config.host),
            wallet_address: Some(init_config.wallet_address),
            mix_port: init_config.mix_port,
            clients_port: init_config.clients_port,
            datastore: init_config.datastore,
            announce_host: init_config.announce_host,
            validator_apis: init_config.validator_apis,
            validators: init_config.validators,
            mnemonic: init_config.mnemonic,

            #[cfg(feature = "coconut")]
            only_coconut_credentials: init_config.only_coconut_credentials,

            enabled_statistics: init_config.enabled_statistics,
            statistics_service_url: init_config.statistics_service_url,
        }
    }
}

pub async fn execute(args: &Init) {
    println!("Initialising gateway {}...", args.id);

    let already_init = if Config::default_config_file_path(Some(&args.id)).exists() {
        println!(
            "Gateway \"{}\" was already initialised before! Config information will be \
            overwritten (but keys will be kept)!",
            args.id
        );
        true
    } else {
        false
    };

    let override_config_fields = OverrideConfig::from(args.clone());

    // Initialising the config structure is just overriding a default constructed one
    let config = override_config(Config::new(&args.id), override_config_fields);

    // if gateway was already initialised, don't generate new keys
    if !already_init {
        let mut rng = rand::rngs::OsRng;

        let identity_keys = identity::KeyPair::new(&mut rng);
        let sphinx_keys = encryption::KeyPair::new(&mut rng);
        let pathfinder = GatewayPathfinder::new_from_config(&config);
        pemstore::store_keypair(
            &sphinx_keys,
            &pemstore::KeyPairPath::new(
                pathfinder.private_encryption_key().to_owned(),
                pathfinder.public_encryption_key().to_owned(),
            ),
        )
        .expect("Failed to save sphinx keys");

        pemstore::store_keypair(
            &identity_keys,
            &pemstore::KeyPairPath::new(
                pathfinder.private_identity_key().to_owned(),
                pathfinder.public_identity_key().to_owned(),
            ),
        )
        .expect("Failed to save identity keys");

        println!("Saved identity and mixnet sphinx keypairs");
    }

    let config_save_location = config.get_config_file_save_location();
    config
        .save_to_file(None)
        .expect("Failed to save the config file");
    println!("Saved configuration file to {:?}", config_save_location);
    println!("Gateway configuration completed.\n\n\n");

    crate::node::create_gateway(config)
        .await
        .print_node_details();
}

#[cfg(test)]
mod tests {
    use network_defaults::var_names::BECH32_PREFIX;

    use crate::node::{storage::InMemStorage, Gateway};

    use super::*;

    #[tokio::test]
    async fn create_gateway_with_in_mem_storage() {
        let args = Init {
            id: "foo-id".to_string(),
            host: "foo-host".to_string(),
            wallet_address: "n1z9egw0knv47nmur0p8vk4rcx59h9gg4zjx9ede".to_string(),
            mix_port: Some(42),
            clients_port: Some(43),
            announce_host: Some("foo-announce-host".to_string()),
            datastore: Some("foo-datastore".to_string()),
            validator_apis: None,
            validators: None,
            mnemonic: None,
            statistics_service_url: None,
            enabled_statistics: None,
            #[cfg(feature = "coconut")]
            only_coconut_credentials: false,
        };
        std::env::set_var(BECH32_PREFIX, "n");

        let config = Config::new(&args.id);
        let config = override_config(config, OverrideConfig::from(args.clone()));

        let (identity_keys, sphinx_keys) = {
            let mut rng = rand::rngs::OsRng;
            (
                identity::KeyPair::new(&mut rng),
                encryption::KeyPair::new(&mut rng),
            )
        };

        // The test is really if this instantiates with InMemStorage without panics
        let _gateway =
            Gateway::new_from_keys_and_storage(config, identity_keys, sphinx_keys, InMemStorage)
                .await;
    }
}
