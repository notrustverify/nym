// Copyright 2021-2022 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use crate::mixnode::{MixNodeConfigUpdate, MixNodeCostParams};
use crate::reward_params::{
    IntervalRewardParams, IntervalRewardingParamsUpdate, NodeRewardParams, Performance,
    RewardingParams,
};
use crate::{delegation, ContractStateParams, NodeId, Percent};
use crate::{Gateway, IdentityKey, MixNode};
use cosmwasm_std::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub rewarding_validator_address: String,
    pub vesting_contract_address: String,

    pub rewarding_denom: String,
    pub epochs_in_interval: u32,
    pub epoch_duration: Duration,
    pub rewarding_parameters: InitialRewardingParams,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InitialRewardingParams {
    pub initial_reward_pool: Decimal,
    pub initial_staking_supply: Decimal,

    pub sybil_resistance: Percent,
    pub active_set_work_factor: Decimal,
    pub interval_pool_emission: Percent,

    pub rewarded_set_size: u32,
    pub active_set_size: u32,
}

impl From<InitialRewardingParams> for RewardingParams {
    fn from(init: InitialRewardingParams) -> Self {
        todo!()
        // RewardingParams {
        //     interval: IntervalRewardParams {
        //         reward_pool: Default::default(),
        //         staking_supply: Default::default(),
        //         epoch_reward_budget: Default::default(),
        //         stake_saturation_point: Default::default(),
        //         sybil_resistance_percent: (),
        //         active_set_work_factor: Default::default(),
        //         epochs_in_interval: 0,
        //     },
        //     epoch: EpochRewardParams {
        //         rewarded_set_size: 0,
        //         active_set_size: 0,
        //     },
        // }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // state/sys-params-related
    UpdateRewardingValidatorAddress {
        address: String,
    },
    UpdateContractStateParams {
        updated_parameters: ContractStateParams,
    },
    UpdateActiveSetSize {
        active_set_size: u32,
        force_immediately: bool,
    },
    UpdateRewardingParams {
        updated_params: IntervalRewardingParamsUpdate,
        force_immediately: bool,
    },
    UpdateIntervalConfig {
        epochs_in_interval: u32,
        epoch_duration_secs: u64,
        force_immediately: bool,
    },
    AdvanceCurrentEpoch {
        new_rewarded_set: Vec<NodeId>,
        expected_active_set_size: u32,
    },
    ReconcileEpochEvents {
        limit: Option<u32>,
    },

    // mixnode-related:
    BondMixnode {
        mix_node: MixNode,
        cost_params: MixNodeCostParams,
        owner_signature: String,
    },
    BondMixnodeOnBehalf {
        mix_node: MixNode,
        cost_params: MixNodeCostParams,
        owner_signature: String,
        owner: String,
    },
    UnbondMixnode {},
    UnbondMixnodeOnBehalf {
        owner: String,
    },
    UpdateMixnodeCostParams {
        new_costs: MixNodeCostParams,
    },
    UpdateMixnodeCostParamsOnBehalf {
        new_costs: MixNodeCostParams,
        owner: String,
    },
    UpdateMixnodeConfig {
        new_config: MixNodeConfigUpdate,
    },
    UpdateMixnodeConfigOnBehalf {
        new_config: MixNodeConfigUpdate,
        owner: String,
    },

    // gateway-related:
    BondGateway {
        gateway: Gateway,
        owner_signature: String,
    },
    BondGatewayOnBehalf {
        gateway: Gateway,
        owner: String,
        owner_signature: String,
    },
    UnbondGateway {},
    UnbondGatewayOnBehalf {
        owner: String,
    },

    // delegation-related:
    DelegateToMixnode {
        mix_id: NodeId,
    },
    DelegateToMixnodeOnBehalf {
        mix_id: NodeId,
        delegate: String,
    },
    UndelegateFromMixnode {
        mix_id: NodeId,
    },
    UndelegateFromMixnodeOnBehalf {
        mix_id: NodeId,
        delegate: String,
    },

    // reward-related
    RewardMixnode {
        mix_id: NodeId,
        performance: Performance,
    },
    WithdrawOperatorReward {},
    WithdrawOperatorRewardOnBehalf {
        owner: String,
    },
    WithdrawDelegatorReward {
        mix_id: NodeId,
    },
    WithdrawDelegatorRewardOnBehalf {
        mix_id: NodeId,
        owner: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // state/sys-params-related
    GetContractVersion {},
    GetRewardingValidatorAddress {},
    GetStateParams {},
    GetState {},
    GetRewardingParams {},
    GetCurrentIntervalDetails {},

    // TODO: implement
    GetRewardedSet {},

    // mixnode-related:
    GetMixNodeBonds {
        limit: Option<u32>,
        start_after: Option<NodeId>,
    },
    GetMixNodesDetailed {
        limit: Option<u32>,
        start_after: Option<NodeId>,
    },
    GetUnbondedMixNodes {
        limit: Option<u32>,
        start_after: Option<NodeId>,
    },
    GetOwnedMixnode {
        address: String,
    },
    GetMixnodeDetails {
        mix_id: NodeId,
    },
    GetStakeSaturation {
        mix_id: NodeId,
    },
    GetUnbondedMixNodeInformation {
        mix_id: NodeId,
    },
    GetLayerDistribution {},

    // gateway-related:
    GetGateways {
        start_after: Option<IdentityKey>,
        limit: Option<u32>,
    },
    GetGatewayBond {
        identity: IdentityKey,
    },
    GetOwnedGateway {
        address: String,
    },

    // delegation-related:
    // gets all [paged] delegations associated with particular mixnode
    GetMixnodeDelegations {
        mix_id: NodeId,
        // since `start_after` is user-provided input, we can't use `Addr` as we
        // can't guarantee it's validated.
        start_after: Option<String>,
        limit: Option<u32>,
    },
    // gets all [paged] delegations associated with particular delegator
    GetDelegatorDelegations {
        // since `delegator` and `proxy` are user-provided inputs, we can't use `Addr` as we
        // can't guarantee they're validated.
        delegator: String,
        proxy: Option<String>,
        start_after: Option<NodeId>,
        limit: Option<u32>,
    },
    // gets delegation associated with particular mixnode, delegator pair
    GetDelegationDetails {
        mix_id: NodeId,
        delegator: String,
        proxy: Option<String>,
    },
    // gets all delegations in the system
    GetAllDelegations {
        start_after: Option<delegation::StorageKey>,
        limit: Option<u32>,
    },

    // rewards related
    GetPendingOperatorReward {
        address: String,
    },
    GetPendingMixNodeOperatorReward {
        mix_id: NodeId,
    },
    GetPendingDelegatorReward {
        address: String,
        mix_id: NodeId,
        proxy: Option<String>,
    },

    // TODO: COMPLETELY NOT DEALT WITH YET
    GetPendingDelegationEvents {
        owner_address: String,
        proxy_address: Option<String>,
    },
    // pending-events related
    // TODO: figure out how to handle those gracefully-ish
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct MigrateMsg {}
