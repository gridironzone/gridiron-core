use cosmwasm_schema::write_api;

use gridiron::pair::InstantiateMsg;
use gridiron::pair_bonded::{ExecuteMsg, QueryMsg};

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        query: QueryMsg,
        execute: ExecuteMsg,
    }
}
