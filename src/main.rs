use gumdrop::Options;
use std::{env, path::PathBuf, time::Duration};
use tendermint::Hash;
use tendermint_light_client::{
    builder::{LightClientBuilder, SupervisorBuilder},
    store::memory::MemoryStore,
    supervisor::{Handle as _, Instance},
    verifier::{
        options::Options as LightClientOptions,
        types::{Height, PeerId, TrustThreshold},
    },
};
use tendermint_rpc as rpc;

use base64::encode;
use mysql::{prelude::Queryable, Pool};

use indexer::response::{BlockResults, NameResults};

#[derive(Debug, Options)]
struct CliOptions {
    #[options(help = "print this help message")]
    help: bool,
    #[options(help = "enable verbose output")]
    verbose: bool,

    #[options(command)]
    command: Option<Command>,
}

#[derive(Debug, Options)]
enum Command {
    #[options(help = "run the light client and continuously sync up to the latest block")]
    Sync(SyncOpts),
}

#[derive(Debug, Options)]
struct SyncOpts {
    #[options(help = "show help for this command")]
    help: bool,
    #[options(
        help = "address of the Tendermint node to connect to",
        meta = "ADDR",
        default = "tcp://127.0.0.1:26657"
    )]
    address: tendermint_rpc::Url,
    #[options(
        help = "height of the initial trusted state (optional if store already initialized)",
        meta = "HEIGHT"
    )]
    trusted_height: Option<Height>,
    #[options(
        help = "hash of the initial trusted state (optional if store already initialized)",
        meta = "HASH"
    )]
    trusted_hash: Option<Hash>,
    #[options(
        help = "path to the database folder",
        meta = "PATH",
        default = "./lightstore"
    )]
    db_path: PathBuf,
}

// MYSQL RECORD
#[derive(Debug, PartialEq, Eq)]
struct Record {
    name: String,
    handle: String,
}

fn main() {
    let opts = CliOptions::parse_args_default_or_exit();

    match opts.command {
        None => {
            eprintln!("Please specify a command:");
            eprintln!("{}\n", CliOptions::command_list().unwrap());
            eprintln!("{}\n", CliOptions::usage());
            std::process::exit(1);
        }
        Some(Command::Sync(sync_opts)) => sync_cmd(sync_opts).unwrap_or_else(|e| {
            eprintln!("Command failed: {}", e);
            std::process::exit(1);
        }),
    }
}

fn make_instance(
    peer_id: PeerId,
    addr: tendermint_rpc::Url,
    opts: &SyncOpts,
) -> Result<Instance, Box<dyn std::error::Error>> {
    let light_store = MemoryStore::new();
    let rpc_client = rpc::HttpClient::new(addr).unwrap();
    let options = LightClientOptions {
        trust_threshold: TrustThreshold::default(),
        trusting_period: Duration::from_secs(36000),
        clock_drift: Duration::from_secs(1),
    };

    let builder =
        LightClientBuilder::prod(peer_id, rpc_client, Box::new(light_store), options, None);

    let builder = if let (Some(height), Some(hash)) = (opts.trusted_height, opts.trusted_hash) {
        builder.trust_primary_at(height, hash)
    } else {
        builder.trust_from_store()
    }?;

    Ok(builder.build())
}

fn sync_cmd(opts: SyncOpts) -> Result<(), Box<dyn std::error::Error>> {
    let primary: PeerId = "BADFADAD0BEFEEDC0C0ADEADBEEFC0FFEEFACADE".parse().unwrap();
    let witness: PeerId = "CEFEEDBADFADAD0C0CEEFACADE0ADEADBEEFC0FF".parse().unwrap();

    let primary_addr = opts.address.clone();
    let witness_addr = opts.address.clone();

    let primary_instance = make_instance(primary, primary_addr.clone(), &opts)?;
    let witness_instance = make_instance(witness, witness_addr.clone(), &opts)?;

    let supervisor = SupervisorBuilder::new()
        .primary(primary, primary_addr, primary_instance)
        .witness(witness, witness_addr, witness_instance)
        .build_prod();

    let handle = supervisor.handle();

    std::thread::spawn(|| supervisor.run());

    let mut last_block: u64 = 0;

    let pool = Pool::new(env::var("MYSQL").unwrap().as_str())?;

    let mut conn = pool.get_conn()?;

    loop {
        match handle.verify_to_highest() {
            Ok(light_block) => {
                if light_block.height().value() <= last_block {
                    continue;
                };

                println!("[info] synced to block {}", light_block.height());
                last_block = light_block.height().value();

                let resp = reqwest::blocking::get(format!(
                    "{}{}",
                    "https://rpc.stargaze-apis.com/block_results?height=",
                    light_block.height().to_string()
                ))?
                .json::<BlockResults>()?;

                let txs_results = resp.result.txs_results;
                if let Some(txs_results) = txs_results {
                    for res in txs_results {
                        if res.code.is_ok() {
                            for event in res.events {
                                if event.type_str == "wasm-verify-text-record" {
                                    let result = event
                                        .clone()
                                        .attributes
                                        .into_iter()
                                        .find(|tag| tag.key.to_string() == "_contract_address" && tag.value.to_string() == "stars1fx74nkqkw2748av8j7ew7r3xt9cgjqduwn8m0ur5lhe49uhlsasszc5fhr");

                                    if let Some(_) = result {
                                        let result =
                                            event.clone().attributes.into_iter().find(|tag| {
                                                tag.key.to_string() == "record"
                                                    && tag.value.to_string() == "twitter"
                                            });
                                        if let Some(_) = result {
                                            let result = event
                                                .clone()
                                                .attributes
                                                .into_iter()
                                                .find(|tag| tag.key.to_string() == "result");
                                            let name_result = event
                                                .clone()
                                                .attributes
                                                .into_iter()
                                                .find(|tag| tag.key.to_string() == "name");
                                            if let Some(result) = result {
                                                if let Some(name_result) = name_result {
                                                    print!("{:?}", event);

                                                    let result = result.value.to_string(); // "true"/"false"
                                                    let name = name_result.value.to_string();

                                                    if result == "true" {
                                                        // The handle was verified

                                                        // Get the handle
                                                        let msg = encode(format!(
                                                            "{{\"nft_info\":{{\"token_id\":\"{}\"}}}}",
                                                            name
                                                        ));

                                                        let resp =
                                                            reqwest::blocking::get(format!(
                                                                "{}{}",
                                                                "https://rest.stargaze-apis.com/cosmwasm/wasm/v1/contract/stars1fx74nkqkw2748av8j7ew7r3xt9cgjqduwn8m0ur5lhe49uhlsasszc5fhr/smart/",
                                                                msg
                                                            ))?
                                                            .json::<NameResults>()?;

                                                        let handle = resp
                                                            .data
                                                            .extension
                                                            .records
                                                            .into_iter()
                                                            .find(|record| record.name == "twitter")
                                                            .unwrap()
                                                            .value;

                                                        // INSERT IGNORE will only insert if the item doesn't already exist
                                                        conn.query_drop(format!(
                                                            r"INSERT IGNORE INTO `twitter`
                                                              SET `name` = '{name}',
                                                                  `handle` = '{handle}'
                                                            ",
                                                            name = name,
                                                            handle = handle
                                                        ))?;
                                                    } else {
                                                        // Verification was revoked for a handle
                                                        conn.query_drop(format!(
                                                        r"DELETE FROM `twitter` WHERE `name` = '{name}'",
                                                        name = name
                                                    ))?;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(err) => {
                println!("[error] sync failed: {}", err);
            }
        }

        std::thread::sleep(Duration::from_millis(800));
    }
}
