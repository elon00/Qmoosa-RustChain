use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentTaskTrigger {
    CellSpawn { x: usize, y: usize },
    EpochMilestone { epoch: usize, active_cells: usize },
    EntropyShift { delta: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskEvent {
    pub generation: usize,
    pub trigger: AgentTaskTrigger,
    pub payload: String,
}

pub struct ConwayEngine {
    pub width: usize,
    pub height: usize,
    pub grid: Vec<Vec<bool>>,
    pub generation: usize,
}

impl ConwayEngine {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            grid: vec![vec![false; width]; height],
            generation: 0,
        }
    }

    pub fn seed_glider(&mut self, start_x: usize, start_y: usize) {
        let coords = [
            (start_x + 1, start_y),
            (start_x + 2, start_y + 1),
            (start_x, start_y + 2),
            (start_x + 1, start_y + 2),
            (start_x + 2, start_y + 2),
        ];
        for (x, y) in coords {
            if x < self.width && y < self.height {
                self.grid[y][x] = true;
            }
        }
    }

    pub fn count_neighbors(&self, x: usize, y: usize) -> usize {
        let mut count = 0;
        for dy in [-1isize, 0, 1] {
            for dx in [-1isize, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = ((x as isize + dx + self.width as isize) % self.width as isize) as usize;
                let ny = ((y as isize + dy + self.height as isize) % self.height as isize) as usize;
                if self.grid[ny][nx] {
                    count += 1;
                }
            }
        }
        count
    }

    pub fn step(&mut self) -> Vec<AgentTaskEvent> {
        let mut next_grid = vec![vec![false; self.width]; self.height];
        let mut events = Vec::new();
        let mut active_count = 0;

        for y in 0..self.height {
            for x in 0..self.width {
                let neighbors = self.count_neighbors(x, y);
                let alive = self.grid[y][x];

                // B3/S23 Conway rules
                if alive && (neighbors == 2 || neighbors == 3) {
                    next_grid[y][x] = true;
                    active_count += 1;
                } else if !alive && neighbors == 3 {
                    next_grid[y][x] = true;
                    active_count += 1;
                    events.push(AgentTaskEvent {
                        generation: self.generation + 1,
                        trigger: AgentTaskTrigger::CellSpawn { x, y },
                        payload: format!("Cell spawned at ({}, {}) -> Triggering Agent Swarm Task", x, y),
                    });
                }
            }
        }

        self.grid = next_grid;
        self.generation += 1;

        if self.generation % 10 == 0 {
            events.push(AgentTaskEvent {
                generation: self.generation,
                trigger: AgentTaskTrigger::EpochMilestone {
                    epoch: self.generation / 10,
                    active_cells: active_count,
                },
                payload: format!("Epoch checkpoint {} reached", self.generation / 10),
            });
        }

        events
    }

    pub fn active_cells(&self) -> usize {
        self.grid.iter().map(|row| row.iter().filter(|&&c| c).count()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glider_simulation() {
        let mut engine = ConwayEngine::new(16, 16);
        engine.seed_glider(1, 1);
        assert_eq!(engine.active_cells(), 5);

        let events = engine.step();
        assert!(engine.active_cells() > 0);
        assert_eq!(engine.generation, 1);
        assert!(!events.is_empty());
    }

    #[test]
    fn test_epoch_milestone_trigger() {
        let mut engine = ConwayEngine::new(16, 16);
        engine.seed_glider(2, 2);

        let mut epoch_triggered = false;
        for _ in 0..10 {
            let events = engine.step();
            for ev in events {
                if let AgentTaskTrigger::EpochMilestone { epoch, .. } = ev.trigger {
                    if epoch == 1 {
                        epoch_triggered = true;
                    }
                }
            }
        }
        assert!(epoch_triggered);
    }
}
