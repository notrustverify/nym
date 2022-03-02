#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]

use bip39::Mnemonic;
use std::str::FromStr;
use std::sync::Arc;

use bandwidth_claim_contract::msg::ExecuteMsg;
use tokio::sync::RwLock;
use url::Url;

use coconut_interface::{
  self, hash_to_scalar, Attribute, Base58, Bytable, Credential, Parameters, Signature, Theta,
  VerificationKey,
};
use credentials::coconut::bandwidth::{obtain_signature, BandwidthVoucherAttributes};
use credentials::obtain_aggregate_verification_key;
use validator_client::nymd::{AccountId, CosmosCoin, Decimal, Denom, NymdClient};

struct State {
  signatures: Vec<Signature>,
  last_tx_hash: String,
  n_attributes: u32,
  params: Parameters,
  serial_number: Attribute,
  binding_number: Attribute,
  voucher_value: Attribute,
  voucher_info: Attribute,
  aggregated_verification_key: Option<VerificationKey>,
}

impl State {
  fn init(public_attributes: Vec<Attribute>, private_attributes: Vec<Attribute>) -> State {
    let n_attributes = (public_attributes.len() + private_attributes.len()) as u32;
    let params = Parameters::new(n_attributes).unwrap();
    State {
      signatures: Vec::new(),
      last_tx_hash: String::new(),
      n_attributes,
      params,
      serial_number: private_attributes[0],
      binding_number: private_attributes[1],
      voucher_value: public_attributes[0],
      voucher_info: public_attributes[1],
      aggregated_verification_key: None,
    }
  }
}

fn parse_url_validators(raw: &[String]) -> Result<Vec<Url>, String> {
  let mut parsed_urls = Vec::with_capacity(raw.len());
  for url in raw {
    let parsed_url: Url = url
      .parse()
      .map_err(|err| format!("one of validator urls is malformed - {}", err))?;
    parsed_urls.push(parsed_url)
  }
  Ok(parsed_urls)
}

#[tauri::command]
async fn deposit_funds(state: tauri::State<'_, Arc<RwLock<State>>>) -> Result<String, String> {
  let nymd_url = Url::from_str("http://127.0.0.1:26657").unwrap();
  let mnemonic = Mnemonic::from_str(&"sun surge soon stomach flavor country gorilla dress oblige stamp attract hip soldier agree steel prize nuclear know enjoy arm bargain always theme matter").unwrap();
  let nymd_client = NymdClient::connect_with_mnemonic(
    network_defaults::all::Network::SANDBOX,
    nymd_url.as_ref(),
    None,
    None,
    AccountId::from_str("nymt14hj2tavq8fpesdwxxcu44rty3hh90vhuysqrsr").ok(),
    mnemonic,
    None,
  )
  .expect("Could not create nymd client");
  let req = ExecuteMsg::BuyBandwidth {};
  let funds = vec![CosmosCoin {
    denom: Denom::from_str(network_defaults::sandbox::DENOM).unwrap(),
    amount: Decimal::from(1000000u64),
  }];
  let last_tx_hash = nymd_client
    .execute(
      nymd_client.erc20_bridge_contract_address().unwrap(),
      &req,
      Default::default(),
      "",
      funds,
    )
    .await
    .unwrap()
    .transaction_hash
    .to_string();
  println!("Tx hash: {}", last_tx_hash);
  let mut state = state.write().await;
  state.last_tx_hash = last_tx_hash.clone();
  Ok(last_tx_hash)
}

#[tauri::command]
async fn delete_credential(
  idx: usize,
  state: tauri::State<'_, Arc<RwLock<State>>>,
) -> Result<Vec<Signature>, String> {
  let mut state = state.write().await;
  state.signatures.remove(idx);
  Ok(state.signatures.clone())
}

#[tauri::command]
async fn list_credentials(
  state: tauri::State<'_, Arc<RwLock<State>>>,
) -> Result<Vec<Signature>, String> {
  let state = state.read().await;
  Ok(state.signatures.clone())
}

async fn get_aggregated_verification_key(
  validator_urls: Vec<String>,
  state: tauri::State<'_, Arc<RwLock<State>>>,
) -> Result<VerificationKey, String> {
  if let Some(verification_key) = &state.read().await.aggregated_verification_key {
    return Ok(verification_key.clone());
  }

  let parsed_urls = parse_url_validators(&validator_urls)?;
  let key = obtain_aggregate_verification_key(&parsed_urls)
    .await
    .map_err(|err| format!("failed to obtain aggregate verification key - {:?}", err))?;

  state
    .write()
    .await
    .aggregated_verification_key
    .replace(key.clone());

  Ok(key)
}

async fn prove_credential(
  idx: usize,
  validator_urls: Vec<String>,
  state: tauri::State<'_, Arc<RwLock<State>>>,
) -> Result<Theta, String> {
  let verification_key = get_aggregated_verification_key(validator_urls, state.clone()).await?;
  let state = state.read().await;

  if let Some(signature) = state.signatures.get(idx) {
    match coconut_interface::prove_bandwidth_credential(
      &state.params,
      &verification_key,
      signature,
      state.serial_number,
      state.binding_number,
    ) {
      Ok(theta) => Ok(theta),
      Err(e) => Err(format!("{:?}", e)),
    }
  } else {
    Err("Got invalid Signature idx".to_string())
  }
}

#[tauri::command]
async fn verify_credential(
  idx: usize,
  validator_urls: Vec<String>,
  state: tauri::State<'_, Arc<RwLock<State>>>,
) -> Result<bool, String> {
  // the API needs to be improved but at least it should compile (in theory)
  let verification_key =
    get_aggregated_verification_key(validator_urls.clone(), state.clone()).await?;
  println!("Verification key {:?}", verification_key.to_bs58());
  let theta = prove_credential(idx, validator_urls, state.clone()).await?;

  let state = state.read().await;

  let public_attributes_bytes = vec![
    state.voucher_value.to_byte_vec(),
    state.voucher_info.to_byte_vec(),
  ];

  let credential = Credential::new(
    state.n_attributes,
    theta,
    public_attributes_bytes,
    state
      .signatures
      .get(idx)
      .ok_or("Got invalid signature idx")?,
  );

  Ok(credential.verify(&verification_key))
}

#[tauri::command]
async fn get_credential(
  validator_urls: Vec<String>,
  state: tauri::State<'_, Arc<RwLock<State>>>,
) -> Result<Vec<Signature>, String> {
  let signature = {
    let guard = state.read().await;
    let parsed_urls = parse_url_validators(&validator_urls)?;
    let bandwidth_credential_attributes = BandwidthVoucherAttributes {
      serial_number: guard.serial_number,
      binding_number: guard.binding_number,
      voucher_value: guard.voucher_value,
      voucher_info: guard.voucher_info,
    };

    obtain_signature(
      &guard.params,
      &bandwidth_credential_attributes,
      &parsed_urls,
      guard.last_tx_hash.clone(),
    )
    .await
    .map_err(|err| format!("failed to obtain aggregate signature - {:?}", err))?
  };

  let mut state = state.write().await;
  state.signatures.push(signature);
  Ok(state.signatures.clone())
}

fn main() {
  let params = coconut_interface::Parameters::new(4).unwrap();
  let bandwidth_credential_attributes = BandwidthVoucherAttributes {
    serial_number: params.random_scalar(),
    binding_number: params.random_scalar(),
    voucher_value: Attribute::from(1000000u64),
    voucher_info: hash_to_scalar("BandwidthVoucher"),
  };
  tauri::Builder::default()
    .manage(Arc::new(RwLock::new(State::init(
      bandwidth_credential_attributes.get_public_attributes(),
      bandwidth_credential_attributes.get_private_attributes(),
    ))))
    .invoke_handler(tauri::generate_handler![
      get_credential,
      deposit_funds,
      delete_credential,
      list_credentials,
      verify_credential
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
