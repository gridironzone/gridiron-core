use gridiron_pair_bonded::error::ContractError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api};

/// This structure stores a GRID-xGRID pool's params.
#[cw_serde]
pub struct Params {
    /// GRID token contract address.
    pub grid_addr: Addr,
    /// xGRID token contract address.
    pub xgrid_addr: Addr,
    /// Gridiron Staking contract address.
    pub staking_addr: Addr,
}

/// This structure stores a GRID-xGRID pool's init params.
#[cw_serde]
pub struct InitParams {
    /// GRID token contract address.
    pub grid_addr: String,
    /// xGRID token contract address.
    pub xgrid_addr: String,
    /// Gridiron Staking contract address.
    pub staking_addr: String,
}

impl InitParams {
    pub fn try_into_params(self, api: &dyn Api) -> Result<Params, ContractError> {
        Ok(Params {
            grid_addr: api.addr_validate(&self.grid_addr)?,
            xgrid_addr: api.addr_validate(&self.xgrid_addr)?,
            staking_addr: api.addr_validate(&self.staking_addr)?,
        })
    }
}

/// This structure describes a migration message.
/// We currently take no arguments for migrations.
#[cw_serde]
pub struct MigrateMsg {}
