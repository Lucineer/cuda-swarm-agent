//! # cuda-swarm-agent
//!
//! Autonomous swarm agent — self-contained vessel that joins fleets,
//! participates in deliberation, and coordinates with neighbors.
//!
//! ```bash
//! cargo run -- --id 1 --name "scout" --listen 0.0.0.0:9090
//! ```
//!
//! The repo IS the agent. Clone → build → run → it's alive.


use std::collections::HashMap;

/// Vessel identity — the agent knows who it is.
#[derive(Debug, Clone)]
pub struct VesselIdentity {
    pub id: u64,
    pub name: String,
    pub fleet_id: Option<String>,
    pub version: String,
    pub capabilities: Vec<String>,
    pub born_at: u64,
}

impl VesselIdentity {
    pub fn new(id: u64, name: &str) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        Self { id, name: name.to_string(), fleet_id: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: vec!["swarm".into(), "deliberation".into(), "health_report".into()],
            born_at: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64) }
    }

    pub fn with_fleet(mut self, fleet: &str) -> Self { self.fleet_id = Some(fleet.to_string()); self }
    pub fn with_capabilities(mut self, caps: Vec<String>) -> Self { self.capabilities = caps; self }
    pub fn age_secs(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        (now - self.born_at) / 1000
    }
}

/// Vessel state — what the agent is doing right now.
#[derive(Debug, Clone, PartialEq)]
pub enum VesselState {
    Booting,
    Idle,
    Deliberating { proposal_id: u64 },
    Executing { task: String },
    Observing,
    Sleeping,
    ShuttingDown,
}

/// The agent itself — a self-contained vessel.
pub struct Vessel {
    pub identity: VesselIdentity,
    pub state: VesselState,
    pub confidence: f64,
    pub neighbors: HashMap<u64, f64>, // vessel_id → trust
    pub tasks_completed: u64,
    pub messages_processed: u64,
    pub uptime_ticks: u64,
}

impl Vessel {
    pub fn new(id: u64, name: &str) -> Self {
        Self { identity: VesselIdentity::new(id, name),
            state: VesselState::Booting, confidence: 0.5,
            neighbors: HashMap::new(), tasks_completed: 0,
            messages_processed: 0, uptime_ticks: 0 }
    }

    /// Boot sequence — vessel comes alive.
    pub fn boot(&mut self) -> BootReport {
        self.state = VesselState::Idle;
        self.confidence = 0.7;
        BootReport {
            vessel_id: self.identity.id,
            name: self.identity.name.clone(),
            capabilities: self.identity.capabilities.clone(),
            version: self.identity.version.clone(),
            confidence: self.confidence,
            neighbor_count: self.neighbors.len(),
        }
    }

    /// Receive a message from a neighbor.
    pub fn receive(&mut self, from_id: u64, msg_type: &str, payload: &[u8]) -> Vec<(u64, String, Vec<u8>)> {
        self.messages_processed += 1;
        let mut responses = vec![];

        match msg_type {
            "ping" => {
                // Update trust on contact
                *self.neighbors.entry(from_id).or_insert(0.5) += 0.01;
                responses.push((from_id, "pong".into(), vec![]));
            }
            "propose" => {
                self.state = VesselState::Deliberating { proposal_id: 0 };
                let accept = self.confidence > 0.6;
                self.confidence += if accept { 0.02 } else { -0.02 };
                let response = if accept { "accept" } else { "reject" };
                responses.push((from_id, response.into(), vec![]));
                self.state = VesselState::Idle;
            }
            "task" => {
                let task_name = String::from_utf8_lossy(payload).to_string();
                self.state = VesselState::Executing { task: task_name.clone() };
                // Simulate execution
                self.confidence = (self.confidence + 0.01).min(1.0);
                self.tasks_completed += 1;
                self.state = VesselState::Idle;
                responses.push((from_id, "done".into(), task_name.as_bytes().to_vec()));
            }
            "join" => {
                self.neighbors.insert(from_id, 0.5);
                responses.push((from_id, "welcome".into(), self.identity.name.as_bytes().to_vec()));
            }
            "leave" => {
                self.neighbors.remove(&from_id);
            }
            _ => {}
        }

        responses
    }

    /// Broadcast to all neighbors.
    pub fn broadcast(&self, msg_type: &str) -> Vec<(u64, String, Vec<u8>)> {
        self.neighbors.keys().map(|&id| (id, msg_type.to_string(), vec![])).collect()
    }

    /// Add a neighbor.
    pub fn add_neighbor(&mut self, id: u64, trust: f64) {
        self.neighbors.insert(id, trust.clamp(0.0, 1.0));
    }

    /// Health check.
    pub fn health(&self) -> HealthReport {
        HealthReport {
            vessel_id: self.identity.id,
            state: format!("{:?}", self.state),
            confidence: self.confidence,
            neighbors: self.neighbors.len(),
            tasks_completed: self.tasks_completed,
            messages_processed: self.messages_processed,
            uptime_ticks: self.uptime_ticks,
            age_secs: self.identity.age_secs(),
            healthy: self.confidence > 0.3 && self.state != VesselState::ShuttingDown,
        }
    }

    /// Tick — advance vessel state.
    pub fn tick(&mut self) {
        self.uptime_ticks += 1;
        // Slow confidence decay
        self.confidence = (self.confidence - 0.0001).max(0.1);
    }
}

/// Report from boot sequence.
#[derive(Debug, Clone)]
pub struct BootReport {
    pub vessel_id: u64,
    pub name: String,
    pub capabilities: Vec<String>,
    pub version: String,
    pub confidence: f64,
    pub neighbor_count: usize,
}

/// Health report.
#[derive(Debug, Clone)]
pub struct HealthReport {
    pub vessel_id: u64,
    pub state: String,
    pub confidence: f64,
    pub neighbors: usize,
    pub tasks_completed: u64,
    pub messages_processed: u64,
    pub uptime_ticks: u64,
    pub age_secs: u64,
    pub healthy: bool,
}

/// Fleet — collection of vessels that can coordinate.
pub struct Fleet {
    vessels: HashMap<u64, Vessel>,
}

impl Fleet {
    pub fn new() -> Self { Self { vessels: HashMap::new() } }

    pub fn spawn(&mut self, id: u64, name: &str) -> BootReport {
        let mut vessel = Vessel::new(id, name);
        let report = vessel.boot();
        self.vessels.insert(id, vessel);
        report
    }

    pub fn connect(&mut self, a: u64, b: u64) {
        if let Some(va) = self.vessels.get_mut(&a) { va.add_neighbor(b, 0.5); }
        if let Some(vb) = self.vessels.get_mut(&b) { vb.add_neighbor(a, 0.5); }
    }

    /// Send message between vessels.
    pub fn route(&mut self, from: u64, to: u64, msg_type: &str, payload: &[u8]) -> Vec<(u64, String, Vec<u8>)> {
        let mut all_responses = vec![];
        if let Some(vessel) = self.vessels.get_mut(&to) {
            all_responses = vessel.receive(from, msg_type, payload);
        }
        all_responses
    }

    /// Tick all vessels.
    pub fn tick_all(&mut self) {
        for vessel in self.vessels.values_mut() { vessel.tick(); }
    }

    /// Fleet health summary.
    pub fn fleet_health(&self) -> FleetHealth {
        let healthy = self.vessels.values().filter(|v| v.confidence > 0.3).count();
        let total_conf: f64 = self.vessels.values().map(|v| v.confidence).sum();
        let avg_conf = if self.vessels.is_empty() { 0.0 } else { total_conf / self.vessels.len() as f64 };
        FleetHealth {
            vessel_count: self.vessels.len(),
            healthy,
            avg_confidence: avg_conf,
            total_tasks: self.vessels.values().map(|v| v.tasks_completed).sum(),
            total_messages: self.vessels.values().map(|v| v.messages_processed).sum(),
        }
    }

    pub fn vessel(&self, id: u64) -> Option<&Vessel> { self.vessels.get(&id) }
    pub fn vessel_count(&self) -> usize { self.vessels.len() }
}

#[derive(Debug, Clone)]
pub struct FleetHealth {
    pub vessel_count: usize,
    pub healthy: usize,
    pub avg_confidence: f64,
    pub total_tasks: u64,
    pub total_messages: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot() {
        let mut v = Vessel::new(1, "scout");
        let report = v.boot();
        assert_eq!(report.name, "scout");
        assert_eq!(v.state, VesselState::Idle);
    }

    #[test]
    fn test_ping_pong() {
        let mut v = Vessel::new(1, "a");
        v.boot();
        let responses = v.receive(2, "ping", &[]);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].1, "pong");
    }

    #[test]
    fn test_propose_accept() {
        let mut v = Vessel::new(1, "acceptor");
        v.boot();
        v.confidence = 0.8;
        let responses = v.receive(2, "propose", &[]);
        assert_eq!(responses[0].1, "accept");
    }

    #[test]
    fn test_propose_reject() {
        let mut v = Vessel::new(1, "skeptic");
        v.boot();
        v.confidence = 0.4;
        let responses = v.receive(2, "propose", &[]);
        assert_eq!(responses[0].1, "reject");
    }

    #[test]
    fn test_task_execution() {
        let mut v = Vessel::new(1, "worker");
        v.boot();
        let responses = v.receive(2, "task", b"scan_perimeter");
        assert_eq!(responses[0].1, "done");
        assert_eq!(v.tasks_completed, 1);
    }

    #[test]
    fn test_fleet_spawn() {
        let mut fleet = Fleet::new();
        fleet.spawn(1, "alpha");
        fleet.spawn(2, "beta");
        assert_eq!(fleet.vessel_count(), 2);
    }

    #[test]
    fn test_fleet_routing() {
        let mut fleet = Fleet::new();
        fleet.spawn(1, "a");
        fleet.spawn(2, "b");
        fleet.connect(1, 2);
        let responses = fleet.route(1, 2, "ping", &[]);
        assert_eq!(responses[0].1, "pong");
    }

    #[test]
    fn test_fleet_health() {
        let mut fleet = Fleet::new();
        fleet.spawn(1, "healthy");
        fleet.spawn(2, "sick");
        fleet.tick_all();
        let h = fleet.fleet_health();
        assert_eq!(h.vessel_count, 2);
        assert!(h.avg_confidence > 0.0);
    }

    #[test]
    fn test_neighbor_management() {
        let mut v = Vessel::new(1, "social");
        v.boot();
        v.add_neighbor(2, 0.8);
        v.add_neighbor(3, 0.6);
        assert_eq!(v.neighbors.len(), 2);
        let responses = v.receive(2, "leave", &[]);
        assert_eq!(v.neighbors.len(), 1);
    }

    #[test]
    fn test_health_report() {
        let mut v = Vessel::new(1, "a");
        v.boot();
        v.tasks_completed = 5;
        v.messages_processed = 10;
        let h = v.health();
        assert!(h.healthy);
        assert_eq!(h.tasks_completed, 5);
    }

    #[test]
    fn test_confidence_decay() {
        let mut v = Vessel::new(1, "a");
        v.boot();
        let before = v.confidence;
        v.tick();
        v.tick();
        assert!(v.confidence < before);
    }

    #[test]
    fn test_vessel_identity() {
        let v = VesselIdentity::new(42, "navigator").with_fleet("gamma");
        assert_eq!(v.id, 42);
        assert_eq!(v.fleet_id, Some("gamma".into()));
    }

    #[test]
    fn test_broadcast() {
        let mut v = Vessel::new(1, "a");
        v.boot();
        v.add_neighbor(2, 0.5);
        v.add_neighbor(3, 0.5);
        let bc = v.broadcast("hello");
        assert_eq!(bc.len(), 2);
    }
}
