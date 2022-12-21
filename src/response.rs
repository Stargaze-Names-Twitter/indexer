use serde::Deserialize;
use tendermint_rpc::endpoint::broadcast::tx_commit::TxResult;

#[derive(Deserialize, Debug)]
pub struct BlockResults {
    pub jsonrpc: String,
    pub id: i64,
    pub result: Result,
}

#[derive(Deserialize, Debug)]
pub struct Result {
    pub height: String,
    pub txs_results: Option<Vec<TxResult>>,
}

#[derive(Deserialize, Debug)]
pub struct NameResults {
    pub data: NameData,
}

#[derive(Deserialize, Debug)]
pub struct NameData {
    pub token_uri: String,
    pub extension: NameExtension,
}

#[derive(Deserialize, Debug)]
pub struct Record {
    pub name: String,
    pub value: String,
    pub verified: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct NameExtension {
    pub image_nft: Option<NFT>,
    pub records: Vec<Record>,
}

#[derive(Deserialize, Debug)]
pub struct NFT {
    pub collection: String,
    pub token_id: String,
}
