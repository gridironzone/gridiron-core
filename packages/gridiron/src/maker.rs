use crate::asset::{Asset, AssetInfo};
use crate::factory::UpdateAddr;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128, Uint64};

/// This structure stores the main parameters for the Maker contract.
#[cw_serde]
pub struct Config {
    /// Address that's allowed to set contract parameters
    pub owner: Addr,
    /// The factory contract address
    pub factory_contract: Addr,
    /// The xGRID staking contract address.
    pub staking_contract: Option<Addr>,
    /// Default bridge asset (Terra1 - LUNC, Terra2 - LUNA, etc.)
    pub default_bridge: Option<AssetInfo>,
    /// The vxGRID fee distributor contract address
    pub governance_contract: Option<Addr>,
    /// The percentage of fees that go to the vxGRID fee distributor
    pub governance_percent: Uint64,
    /// The GRID token asset info
    pub grid_token: AssetInfo,
    /// The max spread allowed when swapping fee tokens to GRID
    pub max_spread: Decimal,
    /// The flag which determines whether accrued GRID from fee swaps is being distributed or not
    pub rewards_enabled: bool,
    /// The number of blocks over which GRID that accrued pre-upgrade will be distributed
    pub pre_upgrade_blocks: u64,
    /// The last block until which pre-upgrade GRID will be distributed
    pub last_distribution_block: u64,
    /// The remainder of pre-upgrade GRID to distribute
    pub remainder_reward: Uint128,
    /// The amount of collected GRID before enabling rewards distribution
    pub pre_upgrade_grid_amount: Uint128,
    /// Parameters that describe the second receiver of fees
    pub second_receiver_cfg: Option<SecondReceiverConfig>,
}

/// This structure stores general parameters for the contract.
#[cw_serde]
pub struct InstantiateMsg {
    /// Address that's allowed to change contract parameters
    pub owner: String,
    /// Default bridge asset (Terra1 - LUNC, Terra2 - LUNA, etc.)
    pub default_bridge: Option<AssetInfo>,
    /// The GRID token asset info
    pub grid_token: AssetInfo,
    /// The factory contract address
    pub factory_contract: String,
    /// The xGRID staking contract address. If None then governance_contract must be set with 100% fee.
    pub staking_contract: Option<String>,
    /// The governance contract address (fee distributor for vxGRID)
    pub governance_contract: Option<String>,
    /// The percentage of fees that go to governance_contract
    pub governance_percent: Option<Uint64>,
    /// The maximum spread used when swapping fee tokens to GRID
    pub max_spread: Option<Decimal>,
    /// The second receiver parameters of fees
    pub second_receiver_params: Option<SecondReceiverParams>,
}

/// This structure describes the functions that can be executed in this contract.
#[cw_serde]
pub enum ExecuteMsg {
    /// Collects and swaps fee tokens to GRID
    Collect {
        /// The assets to swap to GRID
        assets: Vec<AssetWithLimit>,
    },
    /// Updates general settings
    UpdateConfig {
        /// The factory contract address
        factory_contract: Option<String>,
        /// The xGRID staking contract address
        staking_contract: Option<String>,
        /// The governance contract address (fee distributor for vxGRID)
        governance_contract: Option<UpdateAddr>,
        /// The percentage of fees that go to governance_contract
        governance_percent: Option<Uint64>,
        /// Basic chain asset (Terra1 - LUNC, Terra2 - LUNA, etc.)
        basic_asset: Option<AssetInfo>,
        /// The maximum spread used when swapping fee tokens to GRID
        max_spread: Option<Decimal>,
        /// The second receiver parameters of fees
        second_receiver_params: Option<SecondReceiverParams>,
    },
    /// Add bridge tokens used to swap specific fee tokens to GRID (effectively declaring a swap route)
    UpdateBridges {
        add: Option<Vec<(AssetInfo, AssetInfo)>>,
        remove: Option<Vec<AssetInfo>>,
    },
    /// Swap fee tokens via bridge assets
    SwapBridgeAssets { assets: Vec<AssetInfo>, depth: u64 },
    /// Distribute GRID to stakers and to governance
    DistributeGrid {},
    /// Creates a request to change the contract's ownership
    ProposeNewOwner {
        /// The newly proposed owner
        owner: String,
        /// The validity period of the proposal to change the owner
        expires_in: u64,
    },
    /// Removes a request to change contract ownership
    DropOwnershipProposal {},
    /// Claims contract ownership
    ClaimOwnership {},
    /// Enables the distribution of current fees accrued in the contract over "blocks" number of blocks
    EnableRewards { blocks: u64 },
}

/// This structure describes the query functions available in the contract.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns information about the maker configs that contains in the [`ConfigResponse`]
    #[returns(ConfigResponse)]
    Config {},
    /// Returns the balance for each asset in the specified input parameters
    #[returns(BalancesResponse)]
    Balances { assets: Vec<AssetInfo> },
    #[returns(Vec<(String, String)>)]
    Bridges {},
}

/// A custom struct that holds contract parameters and is used to retrieve them.
#[cw_serde]
pub struct ConfigResponse {
    /// Address that is allowed to update contract parameters
    pub owner: Addr,
    /// Default bridge (Terra1 - LUNC, Terra2 - LUNA, etc.)
    pub default_bridge: Option<AssetInfo>,
    /// The GRID token asset info
    pub grid_token: AssetInfo,
    /// The factory contract address
    pub factory_contract: Addr,
    /// The xGRID staking contract address
    pub staking_contract: Option<Addr>,
    /// The governance contract address (fee distributor for vxGRID stakers)
    pub governance_contract: Option<Addr>,
    /// The percentage of fees that go to governance_contract
    pub governance_percent: Uint64,
    /// The maximum spread used when swapping fee tokens to GRID
    pub max_spread: Decimal,
    /// The remainder GRID tokens (accrued before the Maker is upgraded) to be distributed to xGRID stakers
    pub remainder_reward: Uint128,
    /// The amount of GRID tokens accrued before upgrading the Maker implementation and enabling reward distribution
    pub pre_upgrade_grid_amount: Uint128,
    /// Parameters that describe the second receiver of fees
    pub second_receiver_cfg: Option<SecondReceiverConfig>,
}

/// A custom struct used to return multiple asset balances.
#[cw_serde]
pub struct BalancesResponse {
    pub balances: Vec<Asset>,
}

/// This structure describes a migration message.
#[cw_serde]
pub struct MigrateMsg {
    pub default_bridge: Option<AssetInfo>,
    pub second_receiver_params: Option<SecondReceiverParams>,
}

/// This struct holds parameters to help with swapping a specific amount of a fee token to GRID.
#[cw_serde]
pub struct AssetWithLimit {
    /// Information about the fee token to swap
    pub info: AssetInfo,
    /// The amount of tokens to swap
    pub limit: Option<Uint128>,
}

/// This structure describes the parameters for updating the second receiver of fees.
#[cw_serde]
pub struct SecondReceiverParams {
    /// The second fee receiver
    pub second_fee_receiver: String,
    /// The percentage of fees that go to the second fee receiver
    pub second_receiver_cut: Uint64,
}

/// This structure stores the parameters for the second receiver of fees.
#[cw_serde]
pub struct SecondReceiverConfig {
    /// The second fee receiver contract address
    pub second_fee_receiver: Addr,
    /// The percentage of fees that go to the second fee receiver
    pub second_receiver_cut: Uint64,
}

/// The maximum allowed second receiver share (percents)
pub const MAX_SECOND_RECEIVER_CUT: Uint64 = Uint64::new(50);
