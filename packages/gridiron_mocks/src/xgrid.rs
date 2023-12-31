use std::fmt::Debug;

use gridiron::{
    asset::AssetInfo,
    token::{InstantiateMsg, MinterResponse},
};
use cosmwasm_std::{Addr, Api, CustomQuery, Storage};
use cw_multi_test::{Bank, ContractWrapper, Distribution, Executor, Gov, Ibc, Module, Staking};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::{gridiron_address, MockToken, WKApp, GRIDIRON};

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
    use gridiron_xgrid_token as cnt;
    let contract = Box::new(ContractWrapper::new_with_empty(
        cnt::contract::execute,
        cnt::contract::instantiate,
        cnt::contract::query,
    ));

    app.borrow_mut().store_code(contract)
}

pub struct MockXgridBuilder<B, A, S, C: Module, X, D, I, G> {
    pub app: WKApp<B, A, S, C, X, D, I, G>,
    pub symbol: String,
}

impl<B, A, S, C, X, D, I, G> MockXgridBuilder<B, A, S, C, X, D, I, G>
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
    pub fn new(app: &WKApp<B, A, S, C, X, D, I, G>, symbol: &str) -> Self {
        Self {
            app: app.clone(),
            symbol: symbol.into(),
        }
    }

    pub fn instantiate(self) -> MockXgrid<B, A, S, C, X, D, I, G> {
        let code_id = store_code(&self.app);
        let gridiron = gridiron_address();

        let address = self
            .app
            .borrow_mut()
            .instantiate_contract(
                code_id,
                gridiron,
                &InstantiateMsg {
                    name: self.symbol.clone(),
                    mint: Some(MinterResponse {
                        minter: GRIDIRON.to_owned(),
                        cap: None,
                    }),
                    symbol: self.symbol.clone(),
                    decimals: 6,
                    marketing: None,
                    initial_balances: vec![],
                },
                &[],
                self.symbol,
                Some(GRIDIRON.to_owned()),
            )
            .unwrap();

        MockXgrid {
            app: self.app.clone(),
            address: address.clone(),
            token: MockToken {
                app: self.app,
                address,
            },
        }
    }
}

pub struct MockXgrid<B, A, S, C: Module, X, D, I, G> {
    pub app: WKApp<B, A, S, C, X, D, I, G>,
    pub address: Addr,
    pub token: MockToken<B, A, S, C, X, D, I, G>,
}

impl<B, A, S, C, X, D, I, G> TryFrom<(WKApp<B, A, S, C, X, D, I, G>, &AssetInfo)>
    for MockXgrid<B, A, S, C, X, D, I, G>
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
    type Error = String;
    fn try_from(
        value: (WKApp<B, A, S, C, X, D, I, G>, &AssetInfo),
    ) -> Result<MockXgrid<B, A, S, C, X, D, I, G>, Self::Error> {
        match value.1 {
            AssetInfo::Token { contract_addr } => Ok(MockXgrid {
                app: value.0.clone(),
                address: contract_addr.clone(),
                token: MockToken {
                    app: value.0,
                    address: contract_addr.clone(),
                },
            }),
            AssetInfo::NativeToken { denom } => Err(format!("{} is native coin!", denom)),
        }
    }
}
