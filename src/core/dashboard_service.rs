use std::collections::BTreeMap;

use serde_json::Value;

use crate::core::rpc_client::{RpcCall, RpcClient, RpcError};

#[derive(Debug, Clone, PartialEq)]
pub struct DashboardSnapshot {
    pub chain: ChainSummary,
    pub mempool: MempoolSummary,
    pub network: NetworkSummary,
    pub traffic: TrafficSummary,
    pub peers: Vec<PeerSummary>,
    pub peer_details: BTreeMap<i64, Value>,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChainSummary {
    pub chain: String,
    pub blocks: u64,
    pub headers: u64,
    pub verification_progress: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MempoolSummary {
    pub transactions: u64,
    pub bytes: u64,
    pub usage: u64,
    pub maxmempool: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkSummary {
    pub version: i64,
    pub subversion: String,
    pub protocol_version: i64,
    pub connections: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrafficSummary {
    pub total_bytes_recv: u64,
    pub total_bytes_sent: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PeerSummary {
    pub id: i64,
    pub addr: String,
    pub subver: String,
    pub network: String,
    pub services: String,
    pub transport_version: u8,
    pub inbound: bool,
    pub connection_type: String,
    pub min_ping: Option<f64>,
    pub ping_time: Option<f64>,
    pub last_send: i64,
    pub last_recv: i64,
    pub last_transaction: i64,
    pub last_block: i64,
    pub conn_time: i64,
    pub is_tx_relay: bool,
    pub is_bip152_hb_to: bool,
    pub is_bip152_hb_from: bool,
    pub addr_processed: i64,
    pub addr_rate_limited: i64,
    pub is_addr_relay_enabled: bool,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DashboardPartialUpdate {
    Mempool(MempoolSummary),
    ChainAndMempool {
        chain: ChainSummary,
        mempool: MempoolSummary,
    },
}

pub struct DashboardService {
    rpc_client: RpcClient,
}

impl DashboardService {
    pub fn new(rpc_client: RpcClient) -> Self {
        Self { rpc_client }
    }

    pub fn fetch_snapshot(&self) -> Result<DashboardSnapshot, RpcError> {
        let calls = vec![
            RpcCall::new(
                serde_json::json!(1),
                "getblockchaininfo",
                serde_json::json!([]),
            ),
            RpcCall::new(
                serde_json::json!(2),
                "getnetworkinfo",
                serde_json::json!([]),
            ),
            RpcCall::new(
                serde_json::json!(3),
                "getmempoolinfo",
                serde_json::json!([]),
            ),
            RpcCall::new(serde_json::json!(4), "getpeerinfo", serde_json::json!([])),
            RpcCall::new(serde_json::json!(5), "uptime", serde_json::json!([])),
            RpcCall::new(serde_json::json!(6), "getnettotals", serde_json::json!([])),
        ];
        let responses = self.rpc_client.batch(&calls)?;
        self.build_snapshot(&responses)
    }

    pub fn fetch_mempool_update(&self) -> Result<DashboardPartialUpdate, RpcError> {
        let calls = vec![RpcCall::new(
            serde_json::json!(1),
            "getmempoolinfo",
            serde_json::json!([]),
        )];
        let responses = self.rpc_client.batch(&calls)?;
        let mempool = responses
            .first()
            .ok_or_else(|| RpcError::InvalidResponse("missing mempool response".to_string()))
            .and_then(parse_mempool)?;
        Ok(DashboardPartialUpdate::Mempool(mempool))
    }

    pub fn fetch_chain_and_mempool_update(&self) -> Result<DashboardPartialUpdate, RpcError> {
        let calls = vec![
            RpcCall::new(
                serde_json::json!(1),
                "getblockchaininfo",
                serde_json::json!([]),
            ),
            RpcCall::new(
                serde_json::json!(2),
                "getmempoolinfo",
                serde_json::json!([]),
            ),
        ];
        let responses = self.rpc_client.batch(&calls)?;
        if responses.len() != 2 {
            return Err(RpcError::InvalidResponse(format!(
                "expected 2 partial responses, got {}",
                responses.len()
            )));
        }

        Ok(DashboardPartialUpdate::ChainAndMempool {
            chain: parse_chain(&responses[0])?,
            mempool: parse_mempool(&responses[1])?,
        })
    }

    pub fn build_snapshot(&self, responses: &[Value]) -> Result<DashboardSnapshot, RpcError> {
        if responses.len() != 6 {
            return Err(RpcError::InvalidResponse(format!(
                "expected 6 dashboard responses, got {}",
                responses.len()
            )));
        }

        let network = responses[1].as_object().ok_or_else(|| {
            RpcError::InvalidResponse("getnetworkinfo result must be object".to_string())
        })?;
        let peers = responses[3].as_array().ok_or_else(|| {
            RpcError::InvalidResponse("getpeerinfo result must be array".to_string())
        })?;
        let uptime_secs = responses[4]
            .as_u64()
            .ok_or_else(|| RpcError::InvalidResponse("uptime result must be u64".to_string()))?;
        let traffic = responses[5].as_object().ok_or_else(|| {
            RpcError::InvalidResponse("getnettotals result must be object".to_string())
        })?;

        let chain = parse_chain(&responses[0])?;
        let mempool = parse_mempool(&responses[2])?;

        let network = NetworkSummary {
            version: field(network, "version", Value::as_i64, "i64")?,
            subversion: string(network, "subversion")?,
            protocol_version: field(network, "protocolversion", Value::as_i64, "i64")?,
            connections: field(network, "connections", Value::as_i64, "i64")?,
        };

        let traffic = TrafficSummary {
            total_bytes_recv: field(traffic, "totalbytesrecv", Value::as_u64, "u64")?,
            total_bytes_sent: field(traffic, "totalbytessent", Value::as_u64, "u64")?,
        };

        let mut peer_summaries = Vec::new();
        let mut peer_details = BTreeMap::new();
        for peer in peers {
            let Some(peer_object) = peer.as_object() else {
                continue;
            };
            let Some(id) = field(peer_object, "id", Value::as_i64, "i64").ok() else {
                continue;
            };

            let transport_version = peer_object
                .get("transport_protocol_type")
                .and_then(Value::as_u64)
                .unwrap_or(1) as u8;

            peer_summaries.push(PeerSummary {
                id,
                addr: string(peer_object, "addr").unwrap_or_else(|_| "?".to_string()),
                subver: string(peer_object, "subver").unwrap_or_else(|_| String::new()),
                network: string(peer_object, "network").unwrap_or_else(|_| String::new()),
                services: format_services(peer_object.get("servicesnames")),
                transport_version,
                inbound: field(peer_object, "inbound", Value::as_bool, "bool").unwrap_or(false),
                connection_type: string(peer_object, "connection_type")
                    .unwrap_or_else(|_| "unknown".to_string()),
                min_ping: peer_object.get("minping").and_then(Value::as_f64),
                ping_time: peer_object.get("pingtime").and_then(Value::as_f64),
                last_send: field(peer_object, "lastsend", Value::as_i64, "i64").unwrap_or(0),
                last_recv: field(peer_object, "lastrecv", Value::as_i64, "i64").unwrap_or(0),
                last_transaction: field(peer_object, "last_transaction", Value::as_i64, "i64")
                    .unwrap_or(0),
                last_block: field(peer_object, "last_block", Value::as_i64, "i64").unwrap_or(0),
                conn_time: field(peer_object, "conntime", Value::as_i64, "i64").unwrap_or(0),
                is_tx_relay: field(peer_object, "relaytxes", Value::as_bool, "bool")
                    .unwrap_or(false),
                is_bip152_hb_to: field(peer_object, "bip152_hb_to", Value::as_bool, "bool")
                    .unwrap_or(false),
                is_bip152_hb_from: field(peer_object, "bip152_hb_from", Value::as_bool, "bool")
                    .unwrap_or(false),
                addr_processed: field(peer_object, "addr_processed", Value::as_i64, "i64")
                    .unwrap_or(0),
                addr_rate_limited: field(peer_object, "addr_rate_limited", Value::as_i64, "i64")
                    .unwrap_or(0),
                is_addr_relay_enabled: field(
                    peer_object,
                    "addr_relay_enabled",
                    Value::as_bool,
                    "bool",
                )
                .unwrap_or(false),
                version: field(peer_object, "version", Value::as_i64, "i64").unwrap_or(0),
            });
            peer_details.insert(id, peer.clone());
        }

        Ok(DashboardSnapshot {
            chain,
            mempool,
            network,
            traffic,
            peers: peer_summaries,
            peer_details,
            uptime_secs,
        })
    }
}

fn format_services(servicesnames: Option<&Value>) -> String {
    let Some(arr) = servicesnames.and_then(Value::as_array) else {
        return String::new();
    };
    arr.iter()
        .filter_map(Value::as_str)
        .map(|name| match name {
            "NETWORK_LIMITED" => 'l',
            "P2P_V2" => '2',
            other => other.chars().next().unwrap_or('?').to_ascii_lowercase(),
        })
        .collect()
}

fn parse_chain(value: &Value) -> Result<ChainSummary, RpcError> {
    let blockchain = value.as_object().ok_or_else(|| {
        RpcError::InvalidResponse("getblockchaininfo result must be object".to_string())
    })?;

    Ok(ChainSummary {
        chain: string(blockchain, "chain")?,
        blocks: field(blockchain, "blocks", Value::as_u64, "u64")?,
        headers: field(blockchain, "headers", Value::as_u64, "u64")?,
        verification_progress: field(blockchain, "verificationprogress", Value::as_f64, "f64")?,
    })
}

fn parse_mempool(value: &Value) -> Result<MempoolSummary, RpcError> {
    let mempool = value.as_object().ok_or_else(|| {
        RpcError::InvalidResponse("getmempoolinfo result must be object".to_string())
    })?;

    Ok(MempoolSummary {
        transactions: field(mempool, "size", Value::as_u64, "u64")?,
        bytes: field(mempool, "bytes", Value::as_u64, "u64")?,
        usage: field(mempool, "usage", Value::as_u64, "u64")?,
        maxmempool: field(mempool, "maxmempool", Value::as_u64, "u64")?,
    })
}

fn field<T>(
    obj: &serde_json::Map<String, Value>,
    key: &str,
    extract: impl FnOnce(&Value) -> Option<T>,
    label: &str,
) -> Result<T, RpcError> {
    obj.get(key)
        .and_then(extract)
        .ok_or_else(|| RpcError::InvalidResponse(format!("missing {label} field: {key}")))
}

fn string(obj: &serde_json::Map<String, Value>, key: &str) -> Result<String, RpcError> {
    field(obj, key, |v| v.as_str().map(str::to_owned), "string")
}

#[cfg(test)]
mod tests {
    use super::{DashboardPartialUpdate, DashboardService, parse_chain, parse_mempool};
    use crate::core::rpc_client::{RpcClient, RpcConfig};

    #[test]
    fn snapshot_builder_maps_representative_payloads() {
        let service = DashboardService::new(RpcClient::new(RpcConfig::default()));
        let responses = vec![
            serde_json::json!({
                "chain": "regtest",
                "blocks": 101,
                "headers": 101,
                "verificationprogress": 1.0
            }),
            serde_json::json!({
                "version": 299900,
                "subversion": "/Satoshi:30.99.0/",
                "protocolversion": 70016,
                "connections": 8
            }),
            serde_json::json!({
                "size": 2,
                "bytes": 1234,
                "usage": 5678,
                "maxmempool": 300000000
            }),
            serde_json::json!([
                {
                    "id": 1,
                    "addr": "127.0.0.1:18444",
                    "subver": "/Satoshi:28.0.0/",
                    "network": "ipv4",
                    "servicesnames": ["NETWORK", "WITNESS", "NETWORK_LIMITED", "P2P_V2"],
                    "transport_protocol_type": 2,
                    "inbound": true,
                    "connection_type": "manual",
                    "minping": 0.0005,
                    "pingtime": 0.001,
                    "lastsend": 1700000100,
                    "lastrecv": 1700000200,
                    "last_transaction": 1700000050,
                    "last_block": 1700000000,
                    "conntime": 1699999000,
                    "relaytxes": true,
                    "bip152_hb_to": false,
                    "bip152_hb_from": true,
                    "addr_processed": 42,
                    "addr_rate_limited": 3,
                    "addr_relay_enabled": true,
                    "version": 70016
                }
            ]),
            serde_json::json!(123),
            serde_json::json!({
                "totalbytesrecv": 1000,
                "totalbytessent": 2000
            }),
        ];

        let snapshot = service
            .build_snapshot(&responses)
            .expect("snapshot should build");

        assert_eq!(snapshot.chain.chain, "regtest");
        assert_eq!(snapshot.chain.blocks, 101);
        assert_eq!(snapshot.network.connections, 8);
        assert_eq!(snapshot.mempool.transactions, 2);
        assert_eq!(snapshot.traffic.total_bytes_sent, 2000);
        assert_eq!(snapshot.uptime_secs, 123);
        assert_eq!(snapshot.peers.len(), 1);
        assert_eq!(snapshot.peers[0].connection_type, "manual");
        assert_eq!(snapshot.peers[0].network, "ipv4");
        assert_eq!(snapshot.peers[0].services, "nwl2");
        assert_eq!(snapshot.peers[0].transport_version, 2);
        assert_eq!(snapshot.peers[0].version, 70016);
        assert!(snapshot.peers[0].is_bip152_hb_from);
        assert!(!snapshot.peers[0].is_bip152_hb_to);
        assert!(snapshot.peer_details.contains_key(&1));
    }

    #[test]
    fn chain_parser_maps_representative_payload() {
        let chain = parse_chain(&serde_json::json!({
            "chain": "regtest",
            "blocks": 5,
            "headers": 7,
            "verificationprogress": 0.91
        }))
        .expect("chain should parse");
        assert_eq!(chain.chain, "regtest");
        assert_eq!(chain.blocks, 5);
        assert_eq!(chain.headers, 7);
        assert_eq!(chain.verification_progress, 0.91);
    }

    #[test]
    fn mempool_partial_update_variant_holds_values() {
        let mempool = parse_mempool(&serde_json::json!({
            "size": 10,
            "bytes": 100,
            "usage": 200,
            "maxmempool": 300
        }))
        .expect("mempool should parse");

        let update = DashboardPartialUpdate::Mempool(mempool.clone());
        match update {
            DashboardPartialUpdate::Mempool(inner) => {
                assert_eq!(inner.transactions, 10);
                assert_eq!(inner.bytes, 100);
                assert_eq!(inner.usage, 200);
                assert_eq!(inner.maxmempool, 300);
            }
            _ => panic!("expected mempool update"),
        }
    }
}
