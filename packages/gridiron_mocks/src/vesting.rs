use std::fmt::Debug;

use crate::{gridiron_address, MockTokenBuilder, WKApp, GRIDIRON};
use gridiron::{
    asset::AssetInfo,
    vesting::QueryMsg,
    vesting::{ConfigResponse, InstantiateMsg},
};
use cosmwasm_std::{Addr, Api, CustomQuery, Storage};
use cw_multi_test::{Bank, ContractWrapper, Distribution, Executor, Gov, Ibc, Module, Staking};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

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
    use gridiron_vesting as cnt;
    let contract = Box::new(ContractWrapper::new_with_empty(
        cnt::contract::execute,
        cnt::contract::instantiate,
        cnt::contract::query,
    ));

    app.borrow_mut().store_code(contract)
}

pub struct MockVestingBuilder<B, A, S, C: Module, X, D, I, G> {
    pub app: WKApp<B, A, S, C, X, D, I, G>,
    pub grid_token: Option<AssetInfo>,
}

impl<B, A, S, C, X, D, I, G> MockVestingBuilder<B, A, S, C, X, D, I, G>
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

    pub fn with_grid_token(mut self, grid_token: &AssetInfo) -> Self {
        self.grid_token = Some(grid_token.clone());
        self
    }

    pub fn instantiate(self) -> MockVesting<B, A, S, C, X, D, I, G> {
        let code_id = store_code(&self.app);
        let gridiron = gridiron_address();

        let grid_token = self.grid_token.unwrap_or_else(|| {
            MockTokenBuilder::new(&self.app, "GRID")
                .instantiate()
                .asset_info()
        });

        let address = self
            .app
            .borrow_mut()
            .instantiate_contract(
                code_id,
                gridiron,
                &InstantiateMsg {
                    owner: GRIDIRON.to_owned(),
                    vesting_token: grid_token,
                },
                &[],
                "Gridiron Vesting",
                Some(GRIDIRON.to_owned()),
            )
            .unwrap();

        MockVesting {
            app: self.app,
            address,
        }
    }
}

pub struct MockVesting<B, A, S, C: Module, X, D, I, G> {
    pub app: WKApp<B, A, S, C, X, D, I, G>,
    pub address: Addr,
}

impl<B, A, S, C, X, D, I, G> MockVesting<B, A, S, C, X, D, I, G>
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
    pub fn vesting_token_info(&self) -> AssetInfo {
        let res: ConfigResponse = self
            .app
            .borrow()
            .wrap()
            .query_wasm_smart(self.address.clone(), &QueryMsg::Config {})
            .unwrap();

        res.vesting_token
    }
}
