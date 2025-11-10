// Routing Logic - Handles intent routing decisions and path finding
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use super::{Intent, RouterError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub source_chain: String,
    pub target_chain: String,
    pub hops: Vec<RouteHop>,
    pub estimated_cost: u64,
    pub estimated_time_ms: u64,
    pub confidence_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHop {
    pub from_chain: String,
    pub to_chain: String,
    pub bridge_type: BridgeType,
    pub cost: u64,
    pub time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BridgeType {
    Native,      // Direct bridge
    Wrapped,     // Token wrapping
    Liquidity,   // Liquidity pool
    Relay,       // Relayer-based
}

pub struct RoutingEngine {
    chain_graph: ChainGraph,
    route_cache: HashMap<(String, String), Route>,
}

struct ChainGraph {
    nodes: HashSet<String>,
    edges: HashMap<String, Vec<ChainConnection>>,
}

#[derive(Debug, Clone)]
struct ChainConnection {
    target: String,
    bridge_type: BridgeType,
    cost: u64,
    latency_ms: u64,
    reliability: f64,
}

impl RoutingEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            chain_graph: ChainGraph {
                nodes: HashSet::new(),
                edges: HashMap::new(),
            },
            route_cache: HashMap::new(),
        };
        
        // Initialize with default chain connections
        engine.initialize_default_connections();
        
        engine
    }
    
    fn initialize_default_connections(&mut self) {
        // Add chains
        let chains = vec!["ethereum", "polkadot", "bitcoin", "cosmos", "sentium"];
        for chain in chains {
            self.chain_graph.nodes.insert(chain.to_string());
        }
        
        // Add connections
        // Ethereum <-> Sentium
        self.add_connection("ethereum", "sentium", BridgeType::Native, 50000, 5000, 0.99);
        self.add_connection("sentium", "ethereum", BridgeType::Native, 50000, 5000, 0.99);
        
        // Polkadot <-> Sentium
        self.add_connection("polkadot", "sentium", BridgeType::Native, 30000, 3000, 0.98);
        self.add_connection("sentium", "polkadot", BridgeType::Native, 30000, 3000, 0.98);
        
        // Bitcoin <-> Sentium
        self.add_connection("bitcoin", "sentium", BridgeType::Wrapped, 100000, 60000, 0.95);
        self.add_connection("sentium", "bitcoin", BridgeType::Wrapped, 100000, 60000, 0.95);
        
        // Cosmos <-> Sentium
        self.add_connection("cosmos", "sentium", BridgeType::Relay, 40000, 4000, 0.97);
        self.add_connection("sentium", "cosmos", BridgeType::Relay, 40000, 4000, 0.97);
        
        // Ethereum <-> Polkadot (via Sentium)
        self.add_connection("ethereum", "polkadot", BridgeType::Liquidity, 80000, 8000, 0.90);
        self.add_connection("polkadot", "ethereum", BridgeType::Liquidity, 80000, 8000, 0.90);
    }
    
    pub fn add_connection(
        &mut self,
        from: &str,
        to: &str,
        bridge_type: BridgeType,
        cost: u64,
        latency_ms: u64,
        reliability: f64,
    ) {
        self.chain_graph.nodes.insert(from.to_string());
        self.chain_graph.nodes.insert(to.to_string());
        
        let connection = ChainConnection {
            target: to.to_string(),
            bridge_type,
            cost,
            latency_ms,
            reliability,
        };
        
        self.chain_graph.edges
            .entry(from.to_string())
            .or_insert_with(Vec::new)
            .push(connection);
    }
    
    pub fn find_route(&mut self, intent: &Intent) -> Result<Route, RouterError> {
        // Check cache first
        let cache_key = (intent.from_chain.clone(), intent.to_chain.clone());
        if let Some(cached_route) = self.route_cache.get(&cache_key) {
            return Ok(cached_route.clone());
        }
        
        // Find optimal route using Dijkstra's algorithm
        let route = self.find_optimal_path(&intent.from_chain, &intent.to_chain)?;
        
        // Cache the route
        self.route_cache.insert(cache_key, route.clone());
        
        Ok(route)
    }
    
    fn find_optimal_path(&self, source: &str, target: &str) -> Result<Route, RouterError> {
        // Validate chains exist
        if !self.chain_graph.nodes.contains(source) {
            return Err(RouterError::UnsupportedChain(source.to_string()));
        }
        if !self.chain_graph.nodes.contains(target) {
            return Err(RouterError::UnsupportedChain(target.to_string()));
        }
        
        // Direct connection check
        if let Some(connections) = self.chain_graph.edges.get(source) {
            if let Some(direct) = connections.iter().find(|c| c.target == target) {
                return Ok(Route {
                    source_chain: source.to_string(),
                    target_chain: target.to_string(),
                    hops: vec![RouteHop {
                        from_chain: source.to_string(),
                        to_chain: target.to_string(),
                        bridge_type: direct.bridge_type.clone(),
                        cost: direct.cost,
                        time_ms: direct.latency_ms,
                    }],
                    estimated_cost: direct.cost,
                    estimated_time_ms: direct.latency_ms,
                    confidence_score: direct.reliability,
                });
            }
        }
        
        // Multi-hop routing using BFS with cost optimization
        self.find_multi_hop_route(source, target)
    }
    
    fn find_multi_hop_route(&self, source: &str, target: &str) -> Result<Route, RouterError> {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut parent_map: HashMap<String, (String, ChainConnection)> = HashMap::new();
        
        queue.push_back(source.to_string());
        visited.insert(source.to_string());
        
        while let Some(current) = queue.pop_front() {
            if current == target {
                // Reconstruct path
                return self.reconstruct_route(source, target, &parent_map);
            }
            
            if let Some(connections) = self.chain_graph.edges.get(&current) {
                for conn in connections {
                    if !visited.contains(&conn.target) {
                        visited.insert(conn.target.clone());
                        parent_map.insert(conn.target.clone(), (current.clone(), conn.clone()));
                        queue.push_back(conn.target.clone());
                    }
                }
            }
        }
        
        Err(RouterError::TranslationError(
            format!("No route found from {} to {}", source, target)
        ))
    }
    
    fn reconstruct_route(
        &self,
        source: &str,
        target: &str,
        parent_map: &HashMap<String, (String, ChainConnection)>,
    ) -> Result<Route, RouterError> {
        let mut hops = Vec::new();
        let mut current = target.to_string();
        let mut total_cost = 0u64;
        let mut total_time = 0u64;
        let mut min_reliability = 1.0f64;
        
        while current != source {
            if let Some((parent, conn)) = parent_map.get(&current) {
                hops.push(RouteHop {
                    from_chain: parent.clone(),
                    to_chain: current.clone(),
                    bridge_type: conn.bridge_type.clone(),
                    cost: conn.cost,
                    time_ms: conn.latency_ms,
                });
                
                total_cost += conn.cost;
                total_time += conn.latency_ms;
                min_reliability = min_reliability.min(conn.reliability);
                
                current = parent.clone();
            } else {
                return Err(RouterError::TranslationError(
                    "Failed to reconstruct route".to_string()
                ));
            }
        }
        
        hops.reverse();
        
        Ok(Route {
            source_chain: source.to_string(),
            target_chain: target.to_string(),
            hops,
            estimated_cost: total_cost,
            estimated_time_ms: total_time,
            confidence_score: min_reliability,
        })
    }
    
    pub fn get_all_routes(&self, source: &str, target: &str, max_hops: usize) -> Vec<Route> {
        // Find all possible routes up to max_hops
        let mut routes = Vec::new();
        let mut current_path = Vec::new();
        let mut visited = HashSet::new();
        
        self.dfs_find_routes(
            source,
            target,
            &mut current_path,
            &mut visited,
            &mut routes,
            max_hops,
        );
        
        // Sort by cost
        routes.sort_by(|a, b| a.estimated_cost.cmp(&b.estimated_cost));
        
        routes
    }
    
    fn dfs_find_routes(
        &self,
        current: &str,
        target: &str,
        path: &mut Vec<RouteHop>,
        visited: &mut HashSet<String>,
        routes: &mut Vec<Route>,
        max_hops: usize,
    ) {
        if path.len() >= max_hops {
            return;
        }
        
        if current == target && !path.is_empty() {
            // Found a route
            let total_cost: u64 = path.iter().map(|h| h.cost).sum();
            let total_time: u64 = path.iter().map(|h| h.time_ms).sum();
            
            // Calculate confidence score based on hop reliability
            let mut confidence_score = 1.0f64;
            for hop in path.iter() {
                // Find the connection to get reliability
                if let Some(connections) = self.chain_graph.edges.get(&hop.from_chain) {
                    if let Some(conn) = connections.iter().find(|c| c.target == hop.to_chain) {
                        confidence_score *= conn.reliability;
                    }
                }
            }
            
            routes.push(Route {
                source_chain: path[0].from_chain.clone(),
                target_chain: target.to_string(),
                hops: path.clone(),
                estimated_cost: total_cost,
                estimated_time_ms: total_time,
                confidence_score,
            });
            return;
        }
        
        visited.insert(current.to_string());
        
        if let Some(connections) = self.chain_graph.edges.get(current) {
            for conn in connections {
                if !visited.contains(&conn.target) {
                    path.push(RouteHop {
                        from_chain: current.to_string(),
                        to_chain: conn.target.clone(),
                        bridge_type: conn.bridge_type.clone(),
                        cost: conn.cost,
                        time_ms: conn.latency_ms,
                    });
                    
                    self.dfs_find_routes(&conn.target, target, path, visited, routes, max_hops);
                    
                    path.pop();
                }
            }
        }
        
        visited.remove(current);
    }
}

impl Default for RoutingEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_routing_engine_creation() {
        let engine = RoutingEngine::new();
        assert!(engine.chain_graph.nodes.len() >= 5);
    }
    
    #[test]
    fn test_direct_route() {
        let mut engine = RoutingEngine::new();
        let intent = Intent {
            id: "test-1".to_string(),
            from_chain: "ethereum".to_string(),
            to_chain: "sentium".to_string(),
            action: "transfer".to_string(),
            params: vec![],
            context: vec![],
        };
        
        let route = engine.find_route(&intent);
        assert!(route.is_ok());
        
        let route = route.unwrap();
        assert_eq!(route.hops.len(), 1);
        assert_eq!(route.source_chain, "ethereum");
        assert_eq!(route.target_chain, "sentium");
    }
    
    #[test]
    fn test_unsupported_chain() {
        let mut engine = RoutingEngine::new();
        let intent = Intent {
            id: "test-2".to_string(),
            from_chain: "unknown".to_string(),
            to_chain: "sentium".to_string(),
            action: "transfer".to_string(),
            params: vec![],
            context: vec![],
        };
        
        let route = engine.find_route(&intent);
        assert!(route.is_err());
    }
    
    #[test]
    fn test_all_routes() {
        let engine = RoutingEngine::new();
        let routes = engine.get_all_routes("ethereum", "polkadot", 3);
        
        assert!(routes.len() > 0);
        // Should find at least direct route and multi-hop routes
    }
}
