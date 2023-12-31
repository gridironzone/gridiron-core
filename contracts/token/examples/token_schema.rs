use cosmwasm_schema::write_api;

use gridiron::token::InstantiateMsg;
use cw20_base::msg::{ExecuteMsg, QueryMsg};

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        query: QueryMsg,
        execute: ExecuteMsg,
    }
}
