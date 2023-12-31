use std::fmt::Debug;

use gridiron::{
    staking::{ConfigResponse, Cw20HookMsg, InstantiateMsg, QueryMsg},
    token::ExecuteMsg,
};
use cosmwasm_std::{to_binary, Addr, Api, CustomQuery, Storage, Uint128};
use cw_multi_test::{Bank, ContractWrapper, Distribution, Executor, Gov, Ibc, Module, Staking};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{
    gridiron_address, token::MockTokenOpt, MockToken, MockTokenBuilder, WKApp, GRIDIRON,
};

pub fn store_code<B, A, S, C, X, D, I, G>(app: &WKApp<B, A, S, C, X, D, I, G>) -> u64
where
    B: Bank,
    A: Api,
    S: Storage,
    C: Module,
    X: Staking,
    D: Distribution,
    I: Ibc,
    G: Gov,
    C::ExecT: Clone + Debug + PartialEq + JsonSchema + DeserializeOwned + 'static,
    C::QueryT: CustomQuery + DeserializeOwned + 'static,
{
    use gridiron_staking as cnt;
    let contract = Box::new(
        ContractWrapper::new_with_empty(
            cnt::contract::execute,
            cnt::contract::instantiate,
            cnt::contract::query,
        )
        .with_reply_empty(cnt::contract::reply),
    );

    app.borrow_mut().store_code(contract)
}

pub struct MockStakingBuilder<B, A, S, C: Module, X, D, I, G> {
    pub app: WKApp<B, A, S, C, X, D, I, G>,
    pub grid_token: MockTokenOpt<B, A, S, C, X, D, I, G>,
}

impl<B, A, S, C, X, D, I, G> MockStakingBuilder<B, A, S, C, X, D, I, G>
where
    B: Bank,
    A: Api,
    S: Storage,
    C: Module,
    X: Staking,
    D: Distribution,
    I: Ibc,
    G: Gov,
    C::ExecT: Clone + Debug + PartialEq + JsonSchema + DeserializeOwned + 'static,
    C::QueryT: CustomQuery + DeserializeOwned + 'static,
{
    pub fn new(app: &WKApp<B, A, S, C, X, D, I, G>) -> Self {
        Self {
            app: app.clone(),
            grid_token: None,
        }
    }

    pub fn with_grid_token(mut self, grid_token: &MockToken<B, A, S, C, X, D, I, G>) -> Self {
        self.grid_token = Some(MockToken {
            app: self.app.clone(),
            address: grid_token.address.clone(),
        });
        self
    }

    pub fn instantiate(self) -> MockStaking<B, A, S, C, X, D, I, G> {
        let code_id = store_code(&self.app);
        let gridiron = gridiron_address();

        let grid_token = self
            .grid_token
            .unwrap_or_else(|| MockTokenBuilder::new(&self.app, "GRID").instantiate());

        let token_code_id = crate::xgrid::store_code(&self.app);

        let address = self
            .app
            .borrow_mut()
            .instantiate_contract(
                code_id,
                gridiron,
                &InstantiateMsg {
                    owner: GRIDIRON.to_owned(),
                    marketing: None,
                    token_code_id,
                    deposit_token_addr: grid_token.address.to_string(),
                },
                &[],
                "Gridiron Staking",
                Some(GRIDIRON.to_string()),
            )
            .unwrap();

        MockStaking {
            app: self.app,
            address,
        }
    }
}

pub struct MockStaking<B, A, S, C: Module, X, D, I, G> {
    pub app: WKApp<B, A, S, C, X, D, I, G>,
    pub address: Addr,
}

impl<B, A, S, C, X, D, I, G> MockStaking<B, A, S, C, X, D, I, G>
where
    B: Bank,
    A: Api,
    S: Storage,
    C: Module,
    X: Staking,
    D: Distribution,
    I: Ibc,
    G: Gov,
    C::ExecT: Clone + Debug + PartialEq + JsonSchema + DeserializeOwned + 'static,
    C::QueryT: CustomQuery + DeserializeOwned + 'static,
{
    pub fn grid_token(&self) -> MockToken<B, A, S, C, X, D, I, G> {
        let config: ConfigResponse = self
            .app
            .borrow()
            .wrap()
            .query_wasm_smart(self.address.to_string(), &QueryMsg::Config {})
            .unwrap();

        MockToken {
            app: self.app.clone(),
            address: config.deposit_token_addr,
        }
    }

    pub fn enter(&self, sender: &Addr, amount: Uint128) {
        let grid_token = self.grid_token();
        self.app
            .borrow_mut()
            .execute_contract(
                sender.clone(),
                grid_token.address,
                &ExecuteMsg::Send {
                    amount,
                    msg: to_binary(&Cw20HookMsg::Enter {}).unwrap(),
                    contract: self.address.to_string(),
                },
                &[],
            )
            .unwrap();
    }

    pub fn xgrid_token(&self) -> MockToken<B, A, S, C, X, D, I, G> {
        let config: ConfigResponse = self
            .app
            .borrow()
            .wrap()
            .query_wasm_smart(self.address.to_string(), &QueryMsg::Config {})
            .unwrap();

        MockToken {
            app: self.app.clone(),
            address: config.share_token_addr,
        }
    }

    pub fn leave(&self, sender: &Addr, amount: Uint128) {
        let xgrid_token = self.xgrid_token();
        self.app
            .borrow_mut()
            .execute_contract(
                sender.clone(),
                xgrid_token.address,
                &ExecuteMsg::Send {
                    amount,
                    msg: to_binary(&Cw20HookMsg::Leave {}).unwrap(),
                    contract: self.address.to_string(),
                },
                &[],
            )
            .unwrap();
    }
}
