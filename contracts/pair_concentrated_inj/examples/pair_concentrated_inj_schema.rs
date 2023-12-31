use gridiron::pair::{ExecuteMsg, InstantiateMsg, MigrateMsg};
use gridiron::pair_concentrated::QueryMsg;
use cosmwasm_schema::write_api;

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        query: QueryMsg,
        execute: ExecuteMsg,
        migrate: MigrateMsg
    }
}
