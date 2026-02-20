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
    pub inbound: bool,
    pub connection_type: String,
    pub ping_time: Option<f64>,
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
            version: i64_field(network, "version")?,
            subversion: string(network, "subversion")?,
            protocol_version: i64_field(network, "protocolversion")?,
            connections: i64_field(network, "connections")?,
        };

        let traffic = TrafficSummary {
            total_bytes_recv: u64_field(traffic, "totalbytesrecv")?,
            total_bytes_sent: u64_field(traffic, "totalbytessent")?,
        };

        let mut peer_summaries = Vec::new();
        let mut peer_details = BTreeMap::new();
        for peer in peers {
            let Some(peer_object) = peer.as_object() else {
                continue;
            };
            let Some(id) = i64_field(peer_object, "id").ok() else {
                continue;
            };

            peer_summaries.push(PeerSummary {
                id,
                addr: string(peer_object, "addr").unwrap_or_else(|_| "?".to_string()),
                inbound: bool_field(peer_object, "inbound").unwrap_or(false),
                connection_type: string(peer_object, "connection_type")
                    .unwrap_or_else(|_| "unknown".to_string()),
                ping_time: peer_object.get("pingtime").and_then(Value::as_f64),
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

fn parse_chain(value: &Value) -> Result<ChainSummary, RpcError> {
    let blockchain = value.as_object().ok_or_else(|| {
        RpcError::InvalidResponse("getblockchaininfo result must be object".to_string())
    })?;

    Ok(ChainSummary {
        chain: string(blockchain, "chain")?,
        blocks: u64_field(blockchain, "blocks")?,
        headers: u64_field(blockchain, "headers")?,
        verification_progress: f64_field(blockchain, "verificationprogress")?,
    })
}

fn parse_mempool(value: &Value) -> Result<MempoolSummary, RpcError> {
    let mempool = value.as_object().ok_or_else(|| {
        RpcError::InvalidResponse("getmempoolinfo result must be object".to_string())
    })?;

    Ok(MempoolSummary {
        transactions: u64_field(mempool, "size")?,
        bytes: u64_field(mempool, "bytes")?,
        usage: u64_field(mempool, "usage")?,
        maxmempool: u64_field(mempool, "maxmempool")?,
    })
}

fn string(object: &serde_json::Map<String, Value>, key: &str) -> Result<String, RpcError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| RpcError::InvalidResponse(format!("missing string field: {key}")))
}

fn u64_field(object: &serde_json::Map<String, Value>, key: &str) -> Result<u64, RpcError> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| RpcError::InvalidResponse(format!("missing u64 field: {key}")))
}

fn i64_field(object: &serde_json::Map<String, Value>, key: &str) -> Result<i64, RpcError> {
    object
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| RpcError::InvalidResponse(format!("missing i64 field: {key}")))
}

fn f64_field(object: &serde_json::Map<String, Value>, key: &str) -> Result<f64, RpcError> {
    object
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| RpcError::InvalidResponse(format!("missing f64 field: {key}")))
}

fn bool_field(object: &serde_json::Map<String, Value>, key: &str) -> Result<bool, RpcError> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| RpcError::InvalidResponse(format!("missing bool field: {key}")))
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
                    "inbound": true,
                    "connection_type": "manual",
                    "pingtime": 0.001
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
