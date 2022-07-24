import { FeeDetails, DecCoin, MixnodeStatus, TransactionExecuteResult } from '@nymproject/types';
import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import { isMixnode, Network } from 'src/types';
import { TBondGatewayArgs, TBondMixNodeArgs } from 'src/types';
import {
  bondGateway as bondGatewayRequest,
  bondMixNode as bondMixNodeRequest,
  claimOperatorRewards,
  compoundOperatorRewards,
  simulateBondGateway,
  simulateBondMixnode,
  simulateUnbondGateway,
  simulateUnbondMixnode,
  simulateVestingBondGateway,
  simulateVestingBondMixnode,
  simulateVestingUnbondGateway,
  simulateVestingUnbondMixnode,
  simulateUpdateMixnode,
  simulateVestingUpdateMixnode,
  unbondGateway as unbondGatewayRequest,
  unbondMixNode as unbondMixnodeRequest,
  vestingBondGateway,
  vestingBondMixNode,
  vestingUnbondGateway,
  vestingUnbondMixnode,
  updateMixnode as updateMixnodeRequest,
  vestingUpdateMixnode as updateMixnodeVestingRequest,
  getGatewayBondDetails,
  getMixnodeBondDetails,
  getMixnodeStatus,
  getOperatorRewards,
  unbondMixNode,
} from '../requests';
import { useCheckOwnership } from '../hooks/useCheckOwnership';
import { AppContext } from './main';

const bonded: TBondedMixnode = {
  identityKey: 'B2Xx4haarLWMajX8w259oHjtRZsC7nHwagbWrJNiA3QC',
  bond: { denom: 'nym', amount: '1234' },
  delegators: 123,
  nodeRewards: { denom: 'nym', amount: '123' },
  operatorRewards: { denom: 'nym', amount: '12' },
  profitMargin: 10,
  stake: { denom: 'nym', amount: '99' },
  stakeSaturation: 99,
  status: 'active',
};

/* const bounded: BondedMixnode | BondedGateway = {
  bond: { denom: 'nym', amount: '1234' },
  identityKey: 'B2Xx4haarLWMajX8w259oHjtRZsC7nHwagbWrJNiA3QC',
  ip: '1.2.34.5',
  location: 'France',
}; */

// TODO add relevant data
export type TBondedMixnode = {
  identityKey: string;
  stake: DecCoin;
  bond: DecCoin;
  stakeSaturation: number;
  profitMargin: number;
  nodeRewards: DecCoin;
  operatorRewards: DecCoin;
  delegators: number;
  status: MixnodeStatus;
  proxy?: string;
};

// TODO add relevant data
export interface TBondedGateway {
  identityKey: string;
  ip: string;
  bond: DecCoin;
  location?: string; // TODO not yet available, only available in Network Explorer API
  proxy?: string;
}

export type TokenPool = 'locked' | 'balance';

export type TBondingContext = {
  isLoading: boolean;
  error?: string;
  bondedNode?: TBondedMixnode | TBondedGateway;
  refresh: () => Promise<void>;
  bondMixnode: (data: TBondMixNodeArgs, tokenPool: TokenPool) => Promise<TransactionExecuteResult | undefined>;
  bondGateway: (data: TBondGatewayArgs, tokenPool: TokenPool) => Promise<TransactionExecuteResult | undefined>;
  unbond: (fee?: FeeDetails) => Promise<TransactionExecuteResult | undefined>;
  bondMore: (signature: string, additionalBond: DecCoin) => Promise<TransactionExecuteResult | undefined>;
  redeemRewards: () => Promise<TransactionExecuteResult[] | undefined>;
  compoundRewards: () => Promise<TransactionExecuteResult[] | undefined>;
  updateMixnode: (pm: number) => Promise<TransactionExecuteResult | undefined>;
};

const calculateStake = (pledge: DecCoin, delegations: DecCoin) => {
  const total = Number(pledge.amount) + Number(delegations.amount);
  return { amount: total.toString(), denom: pledge.denom };
};

export const BondingContext = createContext<TBondingContext>({
  isLoading: true,
  refresh: async () => undefined,
  bondMixnode: async () => {
    throw new Error('Not implemented');
  },
  bondGateway: async () => {
    throw new Error('Not implemented');
  },
  unbond: async () => {
    throw new Error('Not implemented');
  },
  redeemRewards: async () => {
    throw new Error('Not implemented');
  },
  compoundRewards: async () => {
    throw new Error('Not implemented');
  },
  updateMixnode: async () => {
    throw new Error('Not implemented');
  },
  bondMore(): Promise<TransactionExecuteResult | undefined> {
    throw new Error('Not implemented');
  },
});

export const BondingContextProvider = ({
  network,
  children,
}: {
  network?: Network;
  children?: React.ReactNode;
}): JSX.Element => {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string>();
  const [bondedNode, setBondedNode] = useState<TBondedMixnode | TBondedGateway>();

  const { userBalance, clientDetails } = useContext(AppContext);
  const { ownership, checkOwnership, isLoading: isOwnershipLoading } = useCheckOwnership();

  const isVesting = Boolean(ownership.vestingPledge);

  const resetState = () => {
    setError(undefined);
    setBondedNode(undefined);
  };

  const refresh = async () => {
    setIsLoading(true);
    if (ownership.hasOwnership && ownership.nodeType === 'mixnode' && clientDetails) {
      try {
        const data = await getMixnodeBondDetails();
        const operatorRewards = await getOperatorRewards(clientDetails?.client_address);
        if (data) {
          console.log(data);
          const statusResponse = await getMixnodeStatus(data.mix_node.identity_key);
          setBondedNode({
            identityKey: data.mix_node.identity_key,
            ip: '',
            stake: calculateStake(data.pledge_amount, data.total_delegation),
            bond: data.pledge_amount,
            stakeSaturation: bonded.stakeSaturation,
            profitMargin: data.mix_node.profit_margin_percent,
            status: statusResponse.status,
            nodeRewards: data.accumulated_rewards,
            delegators: bonded.delegators,
            proxy: data.proxy,
            operatorRewards,
          } as TBondedMixnode);
        }
      } catch (e: any) {
        setError(`While fetching current bond state, an error occurred: ${e}`);
      }
      // TODO convert the returned data from the request to `BondedMixnode` type
    }
    // if (ownership.hasOwnership && ownership.nodeType === 'gateway') {
    //   try {
    //     data = await getGatewayBondDetails();
    //     status = data ? await getMixnodeStatus(data?.gateway.identity_key) : undefined;
    //   } catch (e: any) {
    //     setError(`While fetching current bond state, an error occurred: ${e}`);
    //   }
    //   // TODO convert the returned data from the request to `BondedGateway` type
    //   setBondedGateway(bounded);
    // }
    setIsLoading(false);
  };

  useEffect(() => {
    resetState();
    refresh();
  }, [network, ownership]);

  const bondMixnode = async (data: TBondMixNodeArgs, tokenPool: TokenPool) => {
    let tx: TransactionExecuteResult | undefined;
    setIsLoading(true);
    try {
      if (tokenPool === 'balance') {
        tx = await bondMixNodeRequest(data);
        await userBalance.fetchBalance();
      }
      if (tokenPool === 'locked') {
        tx = await vestingBondMixNode(data);
        await userBalance.fetchTokenAllocation();
      }
      return tx;
    } catch (e: any) {
      setError(`an error occurred: ${e}`);
    } finally {
      await checkOwnership();
    }
    return undefined;
  };

  const bondGateway = async (data: TBondGatewayArgs, tokenPool: TokenPool) => {
    let tx: TransactionExecuteResult | undefined;

    setIsLoading(true);
    try {
      if (tokenPool === 'balance') {
        tx = await bondGatewayRequest(data);
        await userBalance.fetchBalance();
      }
      if (tokenPool === 'locked') {
        tx = await vestingBondGateway(data);
        await userBalance.fetchTokenAllocation();
      }
      return tx;
    } catch (e: any) {
      setError(`an error occurred: ${e}`);
    } finally {
      setIsLoading(false);
    }
    return undefined;
  };

  const unbond = async (fee?: FeeDetails) => {
    let tx;
    setIsLoading(true);
    try {
      if (bondedNode && isMixnode(bondedNode) && bondedNode.proxy) tx = await vestingUnbondMixnode(fee?.fee);
      if (bondedNode && isMixnode(bondedNode) && !bondedNode.proxy) tx = await unbondMixnodeRequest(fee?.fee);
    } catch (e) {
      setError(`an error occurred: ${e as string}`);
    } finally {
      await checkOwnership();
      setIsLoading(false);
    }
    return tx;
  };

  const updateMixnode = async (pm: number, fee?: FeeDetails) => {
    let tx;
    setIsLoading(true);
    try {
      // TODO use estimated fee, need requests update
      if (isVesting) tx = await updateMixnodeRequest(pm, fee?.fee);
      if (!isVesting) tx = await updateMixnodeVestingRequest(pm, fee?.fee);
    } catch (e: any) {
      setError(`an error occurred: ${e}`);
    } finally {
      setIsLoading(false);
    }
    return tx;
  };

  const redeemRewards = async () => {
    let tx;
    setIsLoading(true);
    try {
      tx = await claimOperatorRewards(); // TODO use estimated fee, update `claimOperatorRewards`
    } catch (e: any) {
      setError(`an error occurred: ${e}`);
    } finally {
      setIsLoading(false);
    }
    return tx;
  };

  const compoundRewards = async () => {
    let tx;
    setIsLoading(true);
    try {
      tx = await compoundOperatorRewards(); // TODO use estimated fee, update `compoundOperatorRewards`
    } catch (e: any) {
      setError(`an error occurred: ${e}`);
    } finally {
      setIsLoading(false);
    }
    return tx;
  };

  const bondMore = async (_signature: string, _additionalBond: DecCoin) =>
    // TODO to implement
    undefined;

  const memoizedValue = useMemo(
    () => ({
      isLoading: isLoading || isOwnershipLoading,
      error,
      bondMixnode,
      bondedNode,
      bondGateway,
      unbond,
      updateMixnode,
      refresh,
      redeemRewards,
      compoundRewards,
      bondMore,
    }),
    [isLoading, isOwnershipLoading, error, bondedNode, isVesting],
  );

  return <BondingContext.Provider value={memoizedValue}>{children}</BondingContext.Provider>;
};

export const useBondingContext = () => useContext<TBondingContext>(BondingContext);
