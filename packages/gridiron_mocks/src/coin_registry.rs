use std::fmt::Debug;

use gridiron::native_coin_registry::{ExecuteMsg, InstantiateMsg};
use cosmwasm_std::{Addr, Api, CustomQuery, Storage};
use cw_multi_test::{
    AppResponse, Bank, ContractWrapper, Distribution, Executor, Gov, Ibc, Module, Staking,
};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{gridiron_address, WKApp, GRIDIRON};

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
    use gridiron_native_coin_registry as cnt;
    let contract = Box::new(ContractWrapper::new_with_empty(
        cnt::contract::execute,
        cnt::contract::instantiate,
        cnt::contract::query,
    ));

    app.borrow_mut().store_code(contract)
}

pub struct MockCoinRegistryBuilder<B, A, S, C: Module, X, D, I, G> {
    pub app: WKApp<B, A, S, C, X, D, I, G>,
}

impl<B, A, S, C, X, D, I, G> MockCoinRegistryBuilder<B, A, S, C, X, D, I, G>
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
        Self { app: app.clone() }
    }
    pub fn instantiate(self) -> MockCoinRegistry<B, A, S, C, X, D, I, G> {
        let code_id = store_code(&self.app);
        let gridiron = gridiron_address();

        let address = self
            .app
            .borrow_mut()
            .instantiate_contract(
                code_id,
                gridiron.clone(),
                &InstantiateMsg {
                    owner: GRIDIRON.to_owned(),
                },
                &[],
                "Gridiron Coin Registry",
                Some(GRIDIRON.to_owned()),
            )
            .unwrap();

        self.app
            .borrow_mut()
            .execute_contract(
                gridiron,
                address.clone(),
                &ExecuteMsg::Add {
                    native_coins: vec![("ustake".to_owned(), 6), ("ucosmos".to_owned(), 6)],
                },
                &[],
            )
            .unwrap();

        MockCoinRegistry {
            app: self.app,
            address,
        }
    }
}

pub struct MockCoinRegistry<B, A, S, C: Module, X, D, I, G> {
    pub app: WKApp<B, A, S, C, X, D, I, G>,
    pub address: Addr,
}

impl<B, A, S, C, X, D, I, G> MockCoinRegistry<B, A, S, C, X, D, I, G>
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
    pub fn add(&self, coins: Vec<(String, u8)>) -> AppResponse {
        let gridiron = gridiron_address();

        self.app
            .borrow_mut()
            .execute_contract(
                gridiron,
                self.address.clone(),
                &ExecuteMsg::Add {
                    native_coins: coins,
                },
                &[],
            )
            .unwrap()
    }
}
