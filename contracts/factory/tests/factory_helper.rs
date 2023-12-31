#![cfg(not(tarpaulin_include))]

use anyhow::Result as AnyResult;
use gridiron::asset::AssetInfo;
use gridiron::factory::{PairConfig, PairType};
use cosmwasm_std::{Addr, Binary};
use cw20::MinterResponse;
use cw_multi_test::{App, AppResponse, ContractWrapper, Executor};

pub struct FactoryHelper {
    pub owner: Addr,
    pub grid_token: Addr,
    pub factory: Addr,
    pub cw20_token_code_id: u64,
}

impl FactoryHelper {
    pub fn init(router: &mut App, owner: &Addr) -> Self {
        let grid_token_contract = Box::new(ContractWrapper::new_with_empty(
            gridiron_token::contract::execute,
            gridiron_token::contract::instantiate,
            gridiron_token::contract::query,
        ));

        let cw20_token_code_id = router.store_code(grid_token_contract);

        let msg = gridiron::token::InstantiateMsg {
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
                cw20_token_code_id,
                owner.clone(),
                &msg,
                &[],
                String::from("GRID"),
                None,
            )
            .unwrap();

        let pair_contract = Box::new(
            ContractWrapper::new_with_empty(
                gridiron_pair::contract::execute,
                gridiron_pair::contract::instantiate,
                gridiron_pair::contract::query,
            )
            .with_reply_empty(gridiron_pair::contract::reply),
        );

        let pair_code_id = router.store_code(pair_contract);

        let factory_contract = Box::new(
            ContractWrapper::new_with_empty(
                gridiron_factory::contract::execute,
                gridiron_factory::contract::instantiate,
                gridiron_factory::contract::query,
            )
            .with_reply_empty(gridiron_factory::contract::reply),
        );

        let factory_code_id = router.store_code(factory_contract);

        let msg = gridiron::factory::InstantiateMsg {
            pair_configs: vec![PairConfig {
                code_id: pair_code_id,
                pair_type: PairType::Xyk {},
                total_fee_bps: 100,
                maker_fee_bps: 10,
                is_disabled: false,
                is_generator_disabled: false,
            }],
            token_code_id: cw20_token_code_id,
            fee_address: None,
            generator_address: None,
            owner: owner.to_string(),
            whitelist_code_id: 0,
            coin_registry_address: "coin_registry".to_string(),
        };

        let factory = router
            .instantiate_contract(
                factory_code_id,
                owner.clone(),
                &msg,
                &[],
                String::from("GRID"),
                None,
            )
            .unwrap();

        Self {
            owner: owner.clone(),
            grid_token,
            factory,
            cw20_token_code_id,
        }
    }

    pub fn update_config(
        &mut self,
        router: &mut App,
        sender: &Addr,
        token_code_id: Option<u64>,
        fee_address: Option<String>,
        generator_address: Option<String>,
        whitelist_code_id: Option<u64>,
        coin_registry_address: Option<String>,
    ) -> AnyResult<AppResponse> {
        let msg = gridiron::factory::ExecuteMsg::UpdateConfig {
            token_code_id,
            fee_address,
            generator_address,
            whitelist_code_id,
            coin_registry_address,
        };

        router.execute_contract(sender.clone(), self.factory.clone(), &msg, &[])
    }

    pub fn create_pair(
        &mut self,
        router: &mut App,
        sender: &Addr,
        pair_type: PairType,
        tokens: [&Addr; 2],
        init_params: Option<Binary>,
    ) -> AnyResult<AppResponse> {
        let asset_infos = vec![
            AssetInfo::Token {
                contract_addr: tokens[0].clone(),
            },
            AssetInfo::Token {
                contract_addr: tokens[1].clone(),
            },
        ];

        let msg = gridiron::factory::ExecuteMsg::CreatePair {
            pair_type,
            asset_infos,
            init_params,
        };

        router.execute_contract(sender.clone(), self.factory.clone(), &msg, &[])
    }
}

pub fn instantiate_token(
    app: &mut App,
    token_code_id: u64,
    owner: &Addr,
    token_name: &str,
    decimals: Option<u8>,
) -> Addr {
    let init_msg = gridiron::token::InstantiateMsg {
        name: token_name.to_string(),
        symbol: token_name.to_string(),
        decimals: decimals.unwrap_or(6),
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    app.instantiate_contract(
        token_code_id,
        owner.clone(),
        &init_msg,
        &[],
        token_name,
        None,
    )
    .unwrap()
}
