"""
AI-Powered Route Optimizer
Uses Graph Neural Networks (GNN) to find optimal cross-chain routes
"""

import torch
import torch.nn as nn
import torch.nn.functional as F
from torch_geometric.nn import GCNConv, global_mean_pool
from torch_geometric.data import Data, Batch
import numpy as np
from typing import List, Tuple, Dict, Optional
import json


class RouteGNN(nn.Module):
    """
    Graph Neural Network for route optimization
    Learns optimal routing patterns from historical data
    """
    
    def __init__(self, node_features: int = 16, hidden_dim: int = 64, num_layers: int = 3):
        super(RouteGNN, self).__init__()
        
        self.node_features = node_features
        self.hidden_dim = hidden_dim
        
        # Graph convolutional layers
        self.conv1 = GCNConv(node_features, hidden_dim)
        self.conv2 = GCNConv(hidden_dim, hidden_dim)
        self.conv3 = GCNConv(hidden_dim, hidden_dim)
        
        # Output layers for route scoring
        self.fc1 = nn.Linear(hidden_dim, 32)
        self.fc2 = nn.Linear(32, 1)  # Route score
        
        self.dropout = nn.Dropout(0.2)
        
    def forward(self, x, edge_index, edge_attr, batch):
        """
        Forward pass
        Args:
            x: Node features [num_nodes, node_features]
            edge_index: Edge connectivity [2, num_edges]
            edge_attr: Edge features [num_edges, edge_features]
            batch: Batch assignment [num_nodes]
        Returns:
            Route scores [batch_size, 1]
        """
        # Graph convolutions
        x = self.conv1(x, edge_index)
        x = F.relu(x)
        x = self.dropout(x)
        
        x = self.conv2(x, edge_index)
        x = F.relu(x)
        x = self.dropout(x)
        
        x = self.conv3(x, edge_index)
        x = F.relu(x)
        
        # Global pooling
        x = global_mean_pool(x, batch)
        
        # Output layers
        x = self.fc1(x)
        x = F.relu(x)
        x = self.dropout(x)
        
        x = self.fc2(x)
        
        return x


class RouteOptimizer:
    """
    AI-powered route optimizer using GNN
    """
    
    def __init__(self, model_path: Optional[str] = None):
        self.device = torch.device('cuda' if torch.cuda.is_available() else 'cpu')
        self.model = RouteGNN().to(self.device)
        
        if model_path:
            self.load_model(model_path)
        
        # Chain metadata
        self.chain_metadata = {
            'ethereum': {'id': 0, 'avg_latency': 15000, 'avg_cost': 50000, 'reliability': 0.99},
            'polkadot': {'id': 1, 'avg_latency': 6000, 'avg_cost': 30000, 'reliability': 0.98},
            'bitcoin': {'id': 2, 'avg_latency': 600000, 'avg_cost': 100000, 'reliability': 0.95},
            'cosmos': {'id': 3, 'avg_latency': 7000, 'avg_cost': 40000, 'reliability': 0.97},
            'sentium': {'id': 4, 'avg_latency': 500, 'avg_cost': 10000, 'reliability': 0.99},
        }
        
    def load_model(self, model_path: str):
        """Load pre-trained model"""
        checkpoint = torch.load(model_path, map_location=self.device)
        self.model.load_state_dict(checkpoint['model_state_dict'])
        self.model.eval()
        
    def save_model(self, model_path: str):
        """Save model checkpoint"""
        torch.save({
            'model_state_dict': self.model.state_dict(),
        }, model_path)
        
    def encode_chain(self, chain_name: str) -> np.ndarray:
        """
        Encode chain as feature vector
        Returns: [16] feature vector
        """
        if chain_name not in self.chain_metadata:
            # Unknown chain - use default features
            return np.zeros(16, dtype=np.float32)
        
        meta = self.chain_metadata[chain_name]
        
        # Create feature vector
        features = np.zeros(16, dtype=np.float32)
        features[0] = meta['id'] / 10.0  # Normalized chain ID
        features[1] = meta['avg_latency'] / 1000000.0  # Normalized latency
        features[2] = meta['avg_cost'] / 100000.0  # Normalized cost
        features[3] = meta['reliability']
        
        # One-hot encoding for chain type (positions 4-8)
        features[4 + meta['id']] = 1.0
        
        return features
        
    def create_graph_from_route(self, route: Dict) -> Data:
        """
        Create PyTorch Geometric graph from route
        Args:
            route: Dict with 'hops' list
        Returns:
            PyG Data object
        """
        hops = route['hops']
        
        # Collect unique chains
        chains = set()
        for hop in hops:
            chains.add(hop['from_chain'])
            chains.add(hop['to_chain'])
        
        chains = sorted(list(chains))
        chain_to_idx = {chain: idx for idx, chain in enumerate(chains)}
        
        # Node features
        node_features = []
        for chain in chains:
            features = self.encode_chain(chain)
            node_features.append(features)
        
        x = torch.tensor(node_features, dtype=torch.float32)
        
        # Edge index and attributes
        edge_index = []
        edge_attr = []
        
        for hop in hops:
            from_idx = chain_to_idx[hop['from_chain']]
            to_idx = chain_to_idx[hop['to_chain']]
            
            edge_index.append([from_idx, to_idx])
            
            # Edge features: [cost, latency, bridge_type_onehot]
            edge_features = [
                hop['cost'] / 100000.0,  # Normalized cost
                hop['time_ms'] / 1000000.0,  # Normalized latency
            ]
            
            # Bridge type one-hot (4 types)
            bridge_types = ['Native', 'Wrapped', 'Liquidity', 'Relay']
            bridge_onehot = [0.0] * 4
            if hop['bridge_type'] in bridge_types:
                bridge_onehot[bridge_types.index(hop['bridge_type'])] = 1.0
            edge_features.extend(bridge_onehot)
            
            edge_attr.append(edge_features)
        
        edge_index = torch.tensor(edge_index, dtype=torch.long).t().contiguous()
        edge_attr = torch.tensor(edge_attr, dtype=torch.float32)
        
        return Data(x=x, edge_index=edge_index, edge_attr=edge_attr)
        
    def score_route(self, route: Dict) -> float:
        """
        Score a route using the GNN model
        Args:
            route: Route dictionary
        Returns:
            Score (higher is better)
        """
        self.model.eval()
        
        with torch.no_grad():
            graph = self.create_graph_from_route(route)
            graph = graph.to(self.device)
            
            # Create batch (single graph)
            batch = torch.zeros(graph.x.size(0), dtype=torch.long, device=self.device)
            
            score = self.model(graph.x, graph.edge_index, graph.edge_attr, batch)
            
            return score.item()
            
    def optimize_route(self, routes: List[Dict]) -> Dict:
        """
        Select optimal route from candidates using GNN
        Args:
            routes: List of route dictionaries
        Returns:
            Best route
        """
        if not routes:
            raise ValueError("No routes provided")
        
        if len(routes) == 1:
            return routes[0]
        
        # Score all routes
        scored_routes = []
        for route in routes:
            score = self.score_route(route)
            scored_routes.append((score, route))
        
        # Sort by score (descending)
        scored_routes.sort(key=lambda x: x[0], reverse=True)
        
        return scored_routes[0][1]
        
    def train_step(self, batch_graphs: List[Data], labels: torch.Tensor) -> float:
        """
        Single training step
        Args:
            batch_graphs: List of graph Data objects
            labels: Target scores [batch_size]
        Returns:
            Loss value
        """
        self.model.train()
        
        # Create batch
        batch = Batch.from_data_list(batch_graphs).to(self.device)
        labels = labels.to(self.device)
        
        # Forward pass
        predictions = self.model(batch.x, batch.edge_index, batch.edge_attr, batch.batch)
        predictions = predictions.squeeze()
        
        # Compute loss (MSE)
        loss = F.mse_loss(predictions, labels)
        
        return loss.item()
        
    def train(self, training_data: List[Tuple[Dict, float]], epochs: int = 100, lr: float = 0.001):
        """
        Train the GNN model
        Args:
            training_data: List of (route, score) tuples
            epochs: Number of training epochs
            lr: Learning rate
        """
        optimizer = torch.optim.Adam(self.model.parameters(), lr=lr)
        
        for epoch in range(epochs):
            total_loss = 0.0
            
            # Create batches
            batch_size = 32
            for i in range(0, len(training_data), batch_size):
                batch_data = training_data[i:i+batch_size]
                
                # Prepare batch
                graphs = [self.create_graph_from_route(route) for route, _ in batch_data]
                labels = torch.tensor([score for _, score in batch_data], dtype=torch.float32)
                
                # Training step
                optimizer.zero_grad()
                
                batch = Batch.from_data_list(graphs).to(self.device)
                labels = labels.to(self.device)
                
                predictions = self.model(batch.x, batch.edge_index, batch.edge_attr, batch.batch)
                predictions = predictions.squeeze()
                
                loss = F.mse_loss(predictions, labels)
                loss.backward()
                optimizer.step()
                
                total_loss += loss.item()
            
            avg_loss = total_loss / (len(training_data) / batch_size)
            
            if (epoch + 1) % 10 == 0:
                print(f"Epoch {epoch+1}/{epochs}, Loss: {avg_loss:.4f}")


def cost_function(route: Dict) -> float:
    """
    Calculate route cost
    Cost = w1*financial_cost + w2*time_cost + w3*reliability_penalty
    """
    w1, w2, w3 = 0.4, 0.3, 0.3
    
    financial_cost = route['estimated_cost'] / 100000.0  # Normalize
    time_cost = route['estimated_time_ms'] / 1000000.0  # Normalize
    reliability_penalty = 1.0 - route['confidence_score']
    
    total_cost = w1 * financial_cost + w2 * time_cost + w3 * reliability_penalty
    
    return total_cost


def generate_training_data(num_samples: int = 1000) -> List[Tuple[Dict, float]]:
    """
    Generate synthetic training data for the GNN
    In production, this would use real historical routing data
    """
    training_data = []
    
    chains = ['ethereum', 'polkadot', 'bitcoin', 'cosmos', 'sentium']
    bridge_types = ['Native', 'Wrapped', 'Liquidity', 'Relay']
    
    for _ in range(num_samples):
        # Generate random route
        num_hops = np.random.randint(1, 4)
        hops = []
        
        current_chain = np.random.choice(chains)
        for _ in range(num_hops):
            next_chain = np.random.choice([c for c in chains if c != current_chain])
            
            hop = {
                'from_chain': current_chain,
                'to_chain': next_chain,
                'bridge_type': np.random.choice(bridge_types),
                'cost': np.random.randint(10000, 100000),
                'time_ms': np.random.randint(1000, 60000),
            }
            hops.append(hop)
            current_chain = next_chain
        
        route = {
            'source_chain': hops[0]['from_chain'],
            'target_chain': hops[-1]['to_chain'],
            'hops': hops,
            'estimated_cost': sum(h['cost'] for h in hops),
            'estimated_time_ms': sum(h['time_ms'] for h in hops),
            'confidence_score': np.random.uniform(0.85, 0.99),
        }
        
        # Calculate score (inverse of cost - lower cost = higher score)
        score = 1.0 / (1.0 + cost_function(route))
        
        training_data.append((route, score))
    
    return training_data


if __name__ == '__main__':
    # Example usage
    print("Initializing Route Optimizer...")
    optimizer = RouteOptimizer()
    
    # Generate training data
    print("Generating training data...")
    training_data = generate_training_data(1000)
    
    # Train model
    print("Training GNN model...")
    optimizer.train(training_data, epochs=50)
    
    # Save model
    print("Saving model...")
    optimizer.save_model('route_optimizer.pth')
    
    # Test optimization
    print("\nTesting route optimization...")
    test_routes = [
        {
            'source_chain': 'ethereum',
            'target_chain': 'polkadot',
            'hops': [
                {'from_chain': 'ethereum', 'to_chain': 'sentium', 'bridge_type': 'Native', 'cost': 50000, 'time_ms': 5000},
                {'from_chain': 'sentium', 'to_chain': 'polkadot', 'bridge_type': 'Native', 'cost': 30000, 'time_ms': 3000},
            ],
            'estimated_cost': 80000,
            'estimated_time_ms': 8000,
            'confidence_score': 0.97,
        },
        {
            'source_chain': 'ethereum',
            'target_chain': 'polkadot',
            'hops': [
                {'from_chain': 'ethereum', 'to_chain': 'polkadot', 'bridge_type': 'Liquidity', 'cost': 80000, 'time_ms': 8000},
            ],
            'estimated_cost': 80000,
            'estimated_time_ms': 8000,
            'confidence_score': 0.90,
        },
    ]
    
    best_route = optimizer.optimize_route(test_routes)
    print(f"Best route: {best_route['source_chain']} -> {best_route['target_chain']}")
    print(f"Hops: {len(best_route['hops'])}")
    print(f"Cost: {best_route['estimated_cost']}")
    print(f"Time: {best_route['estimated_time_ms']}ms")
    
    print("\nRoute optimizer ready!")
