export RPC=https://rpc.stargaze-apis.com:443

export SYNC_INFO=$(curl $RPC/status | jq '.result.sync_info')

export LATEST_HEIGHT=$(echo $SYNC_INFO | jq -r '.latest_block_height')
export LATEST_HASH=$(echo $SYNC_INFO | jq -r '.latest_block_hash')

# You'll need to specify MYSQL=<connection_string> before this
cargo run sync -a $RPC -t $LATEST_HEIGHT -T $LATEST_HASH