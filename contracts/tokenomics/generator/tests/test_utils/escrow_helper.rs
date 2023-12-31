#![cfg(not(tarpaulin_include))]

use anyhow::Result;
use gridiron::{staking as xgrid, token as grid};
use gridiron_governance::voting_escrow::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, LockInfoResponse, QueryMsg, VotingPowerResponse,
};
use gridiron_mocks::cw_multi_test::{App, AppResponse, ContractWrapper, Executor};
use cosmwasm_std::{attr, to_binary, Addr, QueryRequest, StdResult, Uint128, WasmQuery};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, MinterResponse};

pub const MULTIPLIER: u64 = 1000000;

pub struct EscrowHelper {
    pub owner: Addr,
    pub grid_token: Addr,
    pub staking_instance: Addr,
    pub xgrid_token: Addr,
    pub escrow_instance: Addr,
    pub grid_token_code_id: u64,
}

impl EscrowHelper {
    pub fn init(router: &mut App, owner: Addr) -> Self {
        let grid_token_contract = Box::new(ContractWrapper::new_with_empty(
            gridiron_token::contract::execute,
            gridiron_token::contract::instantiate,
            gridiron_token::contract::query,
        ));

        let grid_token_code_id = router.store_code(grid_token_contract);

        let msg = grid::InstantiateMsg {
            name: String::from("Grid token"),
            symbol: String::from("GRID"),
            decimals: 6,
            initial_balances: vec![],
            mint: Some(MinterResponse {
                minter: owner.to_string(),
                cap: None,
            }),
            marketing: None,
        };

        let grid_token = router
            .instantiate_contract(
                grid_token_code_id,
                owner.clone(),
                &msg,
                &[],
                String::from("GRID"),
                None,
            )
            .unwrap();

        let staking_contract = Box::new(
            ContractWrapper::new_with_empty(
                gridiron_staking::contract::execute,
                gridiron_staking::contract::instantiate,
                gridiron_staking::contract::query,
            )
            .with_reply_empty(gridiron_staking::contract::reply),
        );

        let staking_code_id = router.store_code(staking_contract);

        let msg = xgrid::InstantiateMsg {
            owner: owner.to_string(),
            token_code_id: grid_token_code_id,
            deposit_token_addr: grid_token.to_string(),
            marketing: None,
        };
        let staking_instance = router
            .instantiate_contract(
                staking_code_id,
                owner.clone(),
                &msg,
                &[],
                String::from("xGRID"),
                None,
            )
            .unwrap();

        let res = router
            .wrap()
            .query::<xgrid::ConfigResponse>(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: staking_instance.to_string(),
                msg: to_binary(&xgrid::QueryMsg::Config {}).unwrap(),
            }))
            .unwrap();

        let voting_contract = Box::new(ContractWrapper::new_with_empty(
            voting_escrow::contract::execute,
            voting_escrow::contract::instantiate,
            voting_escrow::contract::query,
        ));

        let voting_code_id = router.store_code(voting_contract);

        let msg = InstantiateMsg {
            owner: owner.to_string(),
            guardian_addr: Some("guardian".to_string()),
            deposit_token_addr: res.share_token_addr.to_string(),
            marketing: None,
            logo_urls_whitelist: vec![],
        };
        let voting_instance = router
            .instantiate_contract(
                voting_code_id,
                owner.clone(),
                &msg,
                &[],
                String::from("vxGRID"),
                None,
            )
            .unwrap();

        Self {
            owner,
            xgrid_token: res.share_token_addr,
            grid_token,
            staking_instance,
            escrow_instance: voting_instance,
            grid_token_code_id,
        }
    }

    pub fn mint_xgrid(&self, router: &mut App, to: &str, amount: u64) {
        let amount = amount * MULTIPLIER;
        let msg = Cw20ExecuteMsg::Mint {
            recipient: String::from(to),
            amount: Uint128::from(amount),
        };
        let res = router
            .execute_contract(self.owner.clone(), self.grid_token.clone(), &msg, &[])
            .unwrap();
        assert_eq!(res.events[1].attributes[1], attr("action", "mint"));
        assert_eq!(res.events[1].attributes[2], attr("to", String::from(to)));
        assert_eq!(
            res.events[1].attributes[3],
            attr("amount", Uint128::from(amount))
        );

        let to_addr = Addr::unchecked(to);
        let msg = Cw20ExecuteMsg::Send {
            contract: self.staking_instance.to_string(),
            msg: to_binary(&xgrid::Cw20HookMsg::Enter {}).unwrap(),
            amount: Uint128::from(amount),
        };
        router
            .execute_contract(to_addr, self.grid_token.clone(), &msg, &[])
            .unwrap();
    }

    pub fn check_xgrid_balance(&self, router: &mut App, user: &str, amount: u64) {
        let amount = amount * MULTIPLIER;
        let res: BalanceResponse = router
            .wrap()
            .query_wasm_smart(
                self.xgrid_token.clone(),
                &Cw20QueryMsg::Balance {
                    address: user.to_string(),
                },
            )
            .unwrap();
        assert_eq!(res.balance.u128(), amount as u128);
    }

    pub fn create_lock(
        &self,
        router: &mut App,
        user: &str,
        time: u64,
        amount: f32,
    ) -> Result<AppResponse> {
        let amount = (amount * MULTIPLIER as f32) as u64;
        let cw20msg = Cw20ExecuteMsg::Send {
            contract: self.escrow_instance.to_string(),
            amount: Uint128::from(amount),
            msg: to_binary(&Cw20HookMsg::CreateLock { time }).unwrap(),
        };
        router.execute_contract(
            Addr::unchecked(user),
            self.xgrid_token.clone(),
            &cw20msg,
            &[],
        )
    }

    pub fn extend_lock_amount(
        &self,
        router: &mut App,
        user: &str,
        amount: f32,
    ) -> Result<AppResponse> {
        let amount = (amount * MULTIPLIER as f32) as u64;
        let cw20msg = Cw20ExecuteMsg::Send {
            contract: self.escrow_instance.to_string(),
            amount: Uint128::from(amount),
            msg: to_binary(&Cw20HookMsg::ExtendLockAmount {}).unwrap(),
        };
        router.execute_contract(
            Addr::unchecked(user),
            self.xgrid_token.clone(),
            &cw20msg,
            &[],
        )
    }

    pub fn deposit_for(
        &self,
        router: &mut App,
        from: &str,
        to: &str,
        amount: f32,
    ) -> Result<AppResponse> {
        let amount = (amount * MULTIPLIER as f32) as u64;
        let cw20msg = Cw20ExecuteMsg::Send {
            contract: self.escrow_instance.to_string(),
            amount: Uint128::from(amount),
            msg: to_binary(&Cw20HookMsg::DepositFor {
                user: to.to_string(),
            })
            .unwrap(),
        };
        router.execute_contract(
            Addr::unchecked(from),
            self.xgrid_token.clone(),
            &cw20msg,
            &[],
        )
    }

    pub fn extend_lock_time(&self, router: &mut App, user: &str, time: u64) -> Result<AppResponse> {
        router.execute_contract(
            Addr::unchecked(user),
            self.escrow_instance.clone(),
            &ExecuteMsg::ExtendLockTime { time },
            &[],
        )
    }

    pub fn withdraw(&self, router: &mut App, user: &str) -> Result<AppResponse> {
        router.execute_contract(
            Addr::unchecked(user),
            self.escrow_instance.clone(),
            &ExecuteMsg::Withdraw {},
            &[],
        )
    }

    pub fn update_blacklist(
        &self,
        router: &mut App,
        append_addrs: Option<Vec<String>>,
        remove_addrs: Option<Vec<String>>,
    ) -> Result<AppResponse> {
        router.execute_contract(
            Addr::unchecked("owner"),
            self.escrow_instance.clone(),
            &ExecuteMsg::UpdateBlacklist {
                append_addrs,
                remove_addrs,
            },
            &[],
        )
    }

    pub fn query_user_vp(&self, router: &mut App, user: &str) -> StdResult<f32> {
        router
            .wrap()
            .query_wasm_smart(
                self.escrow_instance.clone(),
                &QueryMsg::UserVotingPower {
                    user: user.to_string(),
                },
            )
            .map(|vp: VotingPowerResponse| vp.voting_power.u128() as f32 / MULTIPLIER as f32)
    }

    pub fn query_user_vp_at(&self, router: &mut App, user: &str, time: u64) -> StdResult<f32> {
        router
            .wrap()
            .query_wasm_smart(
                self.escrow_instance.clone(),
                &QueryMsg::UserVotingPowerAt {
                    user: user.to_string(),
                    time,
                },
            )
            .map(|vp: VotingPowerResponse| vp.voting_power.u128() as f32 / MULTIPLIER as f32)
    }

    pub fn query_user_vp_at_period(
        &self,
        router: &mut App,
        user: &str,
        period: u64,
    ) -> StdResult<f32> {
        router
            .wrap()
            .query_wasm_smart(
                self.escrow_instance.clone(),
                &QueryMsg::UserVotingPowerAtPeriod {
                    user: user.to_string(),
                    period,
                },
            )
            .map(|vp: VotingPowerResponse| vp.voting_power.u128() as f32 / MULTIPLIER as f32)
    }

    pub fn query_total_vp(&self, router: &mut App) -> StdResult<f32> {
        router
            .wrap()
            .query_wasm_smart(self.escrow_instance.clone(), &QueryMsg::TotalVotingPower {})
            .map(|vp: VotingPowerResponse| vp.voting_power.u128() as f32 / MULTIPLIER as f32)
    }

    pub fn query_total_vp_at(&self, router: &mut App, time: u64) -> StdResult<f32> {
        router
            .wrap()
            .query_wasm_smart(
                self.escrow_instance.clone(),
                &QueryMsg::TotalVotingPowerAt { time },
            )
            .map(|vp: VotingPowerResponse| vp.voting_power.u128() as f32 / MULTIPLIER as f32)
    }

    pub fn query_total_vp_at_period(&self, router: &mut App, period: u64) -> StdResult<f32> {
        router
            .wrap()
            .query_wasm_smart(
                self.escrow_instance.clone(),
                &QueryMsg::TotalVotingPowerAtPeriod { period },
            )
            .map(|vp: VotingPowerResponse| vp.voting_power.u128() as f32 / MULTIPLIER as f32)
    }

    pub fn query_lock_info(&self, router: &mut App, user: &str) -> StdResult<LockInfoResponse> {
        router.wrap().query_wasm_smart(
            self.escrow_instance.clone(),
            &QueryMsg::LockInfo {
                user: user.to_string(),
            },
        )
    }
}
