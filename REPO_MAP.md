# REPO_MAP

- Rust sources: 40 files
- Config/docs/scripts: 11 files

### benches/benches.rs
- Functions/methods: criterion_benchmark

### src/app.rs
- Structs: App
- Functions/methods: new, write_store, read_store, index, daemon, update

### src/bin/electrs.rs
- Functions/methods: fetch_from, run_server, main

### src/bin/popular-scripts.rs
- Functions/methods: main

### src/bin/tx-fingerprint-stats.rs
- Functions/methods: main

### src/chain.rs
- Enums: Network
- Functions/methods: magic, is_regtest, address_params, native_asset, pegged_asset, names, genesis_hash, bitcoin_genesis_hash, liquid_genesis_hash, from

### src/config.rs
- Structs: Config, StaticCookie, CookieFile
- Enums: RpcLogging
- Functions/methods: str_to_socketaddr, from_args, cookie_getter, options, from, get_network_subdir, get

### src/daemon.rs
- Structs: BlockchainInfo, NetworkInfo, Connection, Counter, Daemon
- Traits: CookieGetter
- Functions/methods (key): parse_hash, header_from_value, block_from_value, tx_from_value, parse_error_code, parse_jsonrpc_reply, get, tcp_connect, new, reconnect, send, recv, next, list_blk_files, read_blk_file_xor_key, magic, call_jsonrpc, handle_request, retry_request, request, retry_reconnect, requests, requests_iter, getblockchaininfo, getnetworkinfo, getbestblockhash, getblockheader, getblockheaders, getblock, getblock_raw, getblocks, gettransactions_available, gettransaction_raw, getmempooltx, getmempooltxids, … (41 total)

### src/electrum/client.rs
- Functions/methods: try_from

### src/electrum/discovery.rs
- Modules: default_servers, tests
- Structs: DiscoveryManager, Server, HealthCheck, ServerEntry
- Enums: ServerAddr, Service
- Functions/methods: new, add_server_request, add_default_server, get_servers, our_features, run_health_check, save_healthy_service, remove_unhealthy_service, check_server, verify_compatibility, spawn_jobs_thread, feature_strs, resolve, fmt, serialize, is_healthy, should_retry, eq, cmp, partial_cmp, is_remote_addr, test

### src/electrum/discovery/default_servers.rs
- Functions/methods: add_default_servers

### src/electrum/mod.rs
- Modules: server, client, discovery
- Structs: ServerFeatures, ServerPorts, ProtocolVersion
- Functions/methods: get_electrum_height, cmp, partial_cmp, from_str, fmt, serialize, deserialize

### src/electrum/server.rs
- Structs: Connection, GetHistoryResult, RPC, Stats
- Enums: Message, Notification
- Functions/methods (key): hash_from_value, usize_from_value, usize_from_value_or, bool_from_value, bool_from_value_or, get_status_hash, new, blockchain_headers_subscribe, server_version, server_banner, server_features, server_donation_address, server_peers_subscribe, server_add_peer, mempool_get_fee_histogram, blockchain_block_header, blockchain_block_headers, blockchain_estimatefee, blockchain_relayfee, blockchain_scripthash_subscribe, blockchain_scripthash_unsubscribe, blockchain_scripthash_get_balance, blockchain_scripthash_get_history, blockchain_scripthash_listunspent, blockchain_transaction_broadcast, blockchain_transaction_get, blockchain_transaction_get_merkle, blockchain_transaction_id_from_pos, handle_command, update_subscriptions, log_rpc_event, send_values, handle_replies, parse_requests, reader_thread, … (42 total)

### src/elements/asset.rs
- Structs: PeggedAsset, IssuedAsset, AssetRow, IssuingInfo, BurningInfo, IssuedAssetStats, PeggedAssetStats
- Enums: LiquidAsset
- Functions/methods: parse_asset_id, new, supply, precision, index_confirmed_tx_assets, index_mempool_tx_assets, remove_mempool_tx_assets, index_tx_assets, asset_history_row, lookup_asset, get_issuance_entropy, asset_cache_key, asset_cache_row, pegged_asset_stats, issued_asset_stats, chain_asset_stats, chain_asset_stats_delta, mempool_asset_stats, apply_issued_asset_stats, apply_pegged_asset_stats

### src/elements/mod.rs
- Modules: asset, peg, registry, ebcompact
- Structs: IssuanceValue
- Traits: SizeMethod, ScriptMethods, TxidCompat
- Functions/methods: from, total_size, is_p2wpkh, is_p2wsh, is_p2tr, compute_txid

### src/elements/peg.rs
- Structs: PegoutValue, PeginInfo, PegoutInfo
- Functions/methods: get_pegin_data, get_pegout_data, from_txout

### src/elements/registry.rs
- Structs: AssetRegistry, AssetMeta, AssetSorting
- Enums: AssetSortField, AssetSortDir
- Functions/methods: new, get, list, fs_sync, spawn_sync, domain, as_comparator, from_query_params, lc_cmp, lc_cmp_opt

### src/errors.rs
- Functions/methods: from

### src/lib.rs
- Modules: chain, config, daemon, electrum, errors, metrics, new_index, rest, signal, util, elements

### src/metrics.rs
- Structs: Metrics, Stats
- Functions/methods: new, counter, counter_vec, gauge, gauge_vec, histogram, histogram_vec, start, handle_request, parse_stats, start_process_exporter

### src/new_index/db.rs
- Structs: DBRow, ScanIterator, ReverseScanIterator, DB
- Enums: DBFlush
- Functions/methods: next, open, full_compaction, enable_auto_compaction, raw_iterator, iter_scan, iter_scan_from, iter_scan_reverse, write, flush, put, put_sync, get, multi_get, verify_compatibility

### src/new_index/fetch.rs
- Structs: BlockEntry, Fetcher
- Enums: FetchFrom
- Functions/methods: start_fetcher, from, map, bitcoind_fetcher, blkfiles_fetcher, blkfiles_reader, blkfile_apply_xor_key, blkfiles_parser, parse_blocks

### src/new_index/mempool.rs
- Structs: Mempool, TxOverview, BacklogStats
- Functions/methods: new, network, lookup_txn, lookup_raw_txn, lookup_spend, has_spend, get_tx_fee, has_unconfirmed_parents, history, _history, history_txids, utxo, stats, txids, recent_txs_overview, backlog_stats, txids_set, update_backlog_stats, add_by_txid, add, lookup_txo, lookup_txos, remove, asset_history, update, default

### src/new_index/mod.rs
- Modules: db, fetch, mempool, precache, query, schema, zmq

### src/new_index/precache.rs
- Functions/methods: precache, scripthashes_from_file, to_scripthash, address_to_scripthash, compute_script_hash

### src/new_index/query.rs
- Structs: Query
- Functions/methods: new, chain, config, network, mempool, broadcast_raw, utxo, history_txids, stats, lookup_txn, lookup_raw_txn, lookup_txos, lookup_spend, lookup_tx_spends, get_tx_status, get_mempool_tx_fee, has_unconfirmed_parents, estimate_fee, estimate_fee_map, update_fee_estimates, get_relayfee, lookup_asset, list_registry_assets

### src/new_index/schema.rs
- Modules: bench
- Structs: Store, Utxo, SpendingInput, ScriptStats, Indexer, IndexerConfig, ChainQuery, TxRowKey, TxRow, TxConfKey, TxConfRow, TxOutKey, TxOutRow, BlockKey, BlockRow, FundingInfo, SpendingInfo, TxHistoryKey, TxHistoryRow, TxEdgeKey, TxEdgeRow, ScriptCacheKey, StatsCacheRow, UtxoCacheRow, Data
- Enums: TxHistoryInfo
- Traits: GetAmountVal
- Functions/methods (key): open, txstore_db, history_db, cache_db, done_initial_sync, from, default, start_timer, headers_to_add, headers_to_index, start_auto_compactions, get_new_headers, update, add, index, fetch_from, new, network, store, get_block_txids, get_block_meta, get_block_raw, get_block_header, get_mtp, get_block_with_meta, history_iter_scan, history_iter_scan_reverse, history, _history, history_txids, _history_txids, utxo, utxo_delta, stats, stats_delta, … (86 total)

### src/new_index/zmq.rs
- Functions/methods: start

### src/rest.rs
- Modules: tests
- Structs: BlockValue, TransactionValue, TxInValue, TxOutValue, UtxoValue, SpendingValue, Handle, HttpError
- Functions/methods: new, from, default, ttl_by_depth, prepare_txs, start, stop, handle_request, http_message, json_response, blocks, to_scripthash, parse_scripthash, not_found, test_parse_query_param, test_parse_value_param

### src/signal.rs
- Structs: Waiter
- Functions/methods: notify, start, wait

### src/util/bincode.rs
- Functions/methods: serialize_big, deserialize_big, serialize_little, deserialize_little, options, big_endian, little_endian

### src/util/block.rs
- Structs: BlockId, HeaderEntry, HeaderList, HashedHeader, BlockStatus, BlockMeta, BlockHeaderMeta
- Functions/methods: from, new, hash, header, height, fmt, empty, order, apply, header_by_blockhash, header_by_height, equals, tip, len, is_empty, iter, get_mtp, confirmed, orphaned, parse_getblock

### src/util/electrum_merkle.rs
- Functions/methods: get_tx_merkle_proof, get_header_merkle_proof, get_id_from_pos, merklize, create_merkle_branch_and_root

### src/util/fees.rs
- Structs: TxFeeInfo
- Functions/methods: new, get_tx_fee, make_fee_histogram

### src/util/mod.rs
- Modules: block, transaction, script, bincode, electrum_merkle, fees
- Structs: SyncChannel, Channel
- Traits: BoolThen
- Functions/methods: full_hash, new, sender, receiver, into_receiver, unbounded, spawn_thread, and_then, create_socket

### src/util/script.rs
- Structs: InnerScripts
- Traits: ScriptToAsm, ScriptToAddr
- Functions/methods: to_asm, to_address_str, get_innerscripts, construct_witness_script, convert_bits, address_str_to_script

### src/util/transaction.rs
- Structs: TransactionStatus, TxInput
- Functions/methods: from, is_coinbase, has_prevout, is_spendable, extract_tx_prevouts, get_prev_outpoints, serialize_outpoint

### tests/common.rs
- Structs: TestRunner
- Functions/methods: new, node_client, sync, mine, send, send_asset, newaddress, ct_newaddress, init_rest_tester, init_electrum_tester, raw_new_address, generate, init_log, rand_available_addr, from

### tests/electrum.rs
- Modules: common
- Functions/methods: test_electrum

### tests/rest.rs
- Modules: common
- Functions/methods: test_rest

## Config, docs, and scripts

### Cargo.toml
- Project configuration / documentation / automation file.

### rust-toolchain.toml
- Project configuration / documentation / automation file.

### .travis.yml
- Project configuration / documentation / automation file.

### README.md
- Project configuration / documentation / automation file.

### RELEASE-NOTES.md
- Project configuration / documentation / automation file.

### TODO.md
- Project configuration / documentation / automation file.

### doc/schema.md
- Project configuration / documentation / automation file.

### doc/usage.md
- Project configuration / documentation / automation file.

### scripts/run.sh
- Project configuration / documentation / automation file.

### contrib/check-api-stablity.sh
- Project configuration / documentation / automation file.

### .hooks/install.sh
- Project configuration / documentation / automation file.
