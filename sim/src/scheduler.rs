use crate::{AgentID, CarID, CreateCar, CreatePedestrian, PedestrianID};
use derivative::Derivative;
use geom::{Duration, DurationHistogram};
use map_model::IntersectionID;
use serde_derive::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum Command {
    // If true, retry when there's no room to spawn somewhere
    SpawnCar(CreateCar, bool),
    SpawnPed(CreatePedestrian),
    UpdateCar(CarID),
    // Distinguish this from UpdateCar to avoid confusing things
    UpdateLaggyHead(CarID),
    UpdatePed(PedestrianID),
    UpdateIntersection(IntersectionID),
    CheckForGridlock,
    Savestate(Duration),
}

impl Command {
    pub fn update_agent(id: AgentID) -> Command {
        match id {
            AgentID::Car(c) => Command::UpdateCar(c),
            AgentID::Pedestrian(p) => Command::UpdatePed(p),
        }
    }

    pub fn to_type(&self) -> CommandType {
        match self {
            Command::SpawnCar(ref create, _) => CommandType::Car(create.vehicle.id),
            Command::SpawnPed(ref create) => CommandType::Ped(create.id),
            Command::UpdateCar(id) => CommandType::Car(*id),
            Command::UpdateLaggyHead(id) => CommandType::CarLaggyHead(*id),
            Command::UpdatePed(id) => CommandType::Ped(*id),
            Command::UpdateIntersection(id) => CommandType::Intersection(*id),
            Command::CheckForGridlock => CommandType::CheckForGridlock,
            Command::Savestate(_) => CommandType::Savestate,
        }
    }
}

// A smaller version of Command that satisfies many more properties. Only one Command per
// CommandType may exist at a time.
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum CommandType {
    Car(CarID),
    CarLaggyHead(CarID),
    Ped(PedestrianID),
    Intersection(IntersectionID),
    CheckForGridlock,
    Savestate,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct Item {
    time: Duration,
    cmd_type: CommandType,
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Item) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Item) -> Ordering {
        // BinaryHeap is a max-heap, so reverse the comparison to get smallest times first.
        let ord = other.time.cmp(&self.time);
        if ord != Ordering::Equal {
            return ord;
        }
        self.cmd_type.cmp(&other.cmd_type)
    }
}

#[derive(Serialize, Deserialize, Derivative)]
#[derivative(PartialEq)]
pub struct Scheduler {
    // TODO Argh, really?!
    #[derivative(PartialEq = "ignore")]
    items: BinaryHeap<Item>,
    queued_commands: BTreeMap<CommandType, (Command, Duration)>,

    latest_time: Duration,
    #[derivative(PartialEq = "ignore")]
    #[serde(skip_serializing, skip_deserializing)]
    delta_times: DurationHistogram,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            items: BinaryHeap::new(),
            queued_commands: BTreeMap::new(),
            latest_time: Duration::ZERO,
            delta_times: std::default::Default::default(),
        }
    }

    pub fn push(&mut self, time: Duration, cmd: Command) {
        if time < self.latest_time {
            panic!(
                "It's at least {}, so can't schedule a command for {}",
                self.latest_time, time
            );
        }
        self.delta_times.add(time - self.latest_time);

        let cmd_type = cmd.to_type();

        // TODO Combo with entry API
        if let Some((existing_cmd, existing_time)) = self.queued_commands.get(&cmd_type) {
            panic!(
                "Can't push({}, {:?}) because ({}, {:?}) already queued",
                time, cmd, existing_time, existing_cmd
            );
        }
        self.queued_commands.insert(cmd_type.clone(), (cmd, time));
        self.items.push(Item { time, cmd_type });
    }

    // Doesn't touch the histogram. Have to call finalize_batch() after. Only for scheduling lots
    // of stuff at the beginning of a simulation.
    // TODO Phase this out?
    pub fn quick_push(&mut self, time: Duration, cmd: Command) {
        self.push(time, cmd);
    }

    pub fn finalize_batch(&mut self) {}

    pub fn update(&mut self, new_time: Duration, cmd: Command) {
        if new_time < self.latest_time {
            panic!(
                "It's at least {}, so can't schedule a command for {}",
                self.latest_time, new_time
            );
        }

        let cmd_type = cmd.to_type();

        // It's fine if a previous command hasn't actually been scheduled.
        if let Some((existing_cmd, _)) = self.queued_commands.get(&cmd_type) {
            assert_eq!(cmd, *existing_cmd);
        }
        self.queued_commands
            .insert(cmd_type.clone(), (cmd, new_time));
        self.items.push(Item {
            time: new_time,
            cmd_type,
        });
    }

    pub fn cancel(&mut self, cmd: Command) {
        // It's fine if a previous command hasn't actually been scheduled.
        self.queued_commands.remove(&cmd.to_type());
    }

    // This API is safer than handing out a batch of items at a time, because while processing one
    // item, we might change the priority of other items or add new items. Don't make the caller
    // reconcile those changes -- just keep pulling items from here, one at a time.
    pub fn get_next(&mut self, now: Duration) -> Option<(Command, Duration)> {
        loop {
            let next_time = self.items.peek().as_ref()?.time;
            if next_time > now {
                return None;
            }

            self.latest_time = next_time;
            let item = self.items.pop().unwrap();
            if let Some((_, cmd_time)) = self.queued_commands.get(&item.cmd_type) {
                // Command was re-scheduled for later.
                if *cmd_time > next_time {
                    continue;
                }
                return self.queued_commands.remove(&item.cmd_type);
            }
            // If the command was outright canceled, fall-through here and pull from the queue
            // again.
        }
    }

    pub fn describe_stats(&self) -> String {
        format!("delta times for events: {}", self.delta_times.describe())
    }
}
