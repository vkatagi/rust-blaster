use crate::actor;
use crate::game_structs;
use actor::{Actor};

use game_structs::{MainState, InputState, Player};


use serde::{Serialize, Deserialize};


/// New Player "handsake". 
/// Server sends this struct to the player that connects.

#[derive(Debug, Serialize, Deserialize)]
pub struct NetPlayerConnected {
    pub player_index: usize
}
impl NetPlayerConnected {
    pub fn make(player_index: usize) -> NetPlayerConnected {
        NetPlayerConnected {
            player_index: player_index
        }
    }
}


#[derive(Debug, Serialize, Deserialize)]
pub struct NetClientInput {
    pub input_state: InputState,
}

impl NetClientInput {
    pub fn update_main_state(self, player_id: usize, state: &mut MainState) {
        state.players[player_id].input = self.input_state;
    }
    
    pub fn make_from_state(state: &MainState) -> NetClientInput {
        NetClientInput {
            input_state: state.local_input.clone(),
        }
    }
}


///
/// Networking struct that the client receives from the server.
///

#[derive(Debug, Serialize, Deserialize)]
pub struct NetFromServer {
    players: Vec<Player>,
    actors: Vec<Actor>,
    score: i32,
    server_time: f32,
}

impl NetFromServer {
    pub fn make_from_state(state: &MainState) -> NetFromServer {
        let mut actors = Vec::new();
        let mut players = Vec::new();

        for player in &state.players {
            let mut player_clone = player.clone();
            player_clone.actor.pre_serialize();
            players.push(player_clone);
        }

        for rock in &state.rocks {
            actors.push(rock.clone());
        }

        for shot in &state.shots {
            actors.push(shot.clone());
        }

        for actor in &mut actors {
            actor.pre_serialize();
        }
        
        NetFromServer {
            players: players,
            actors: actors,
            score: state.score,
            server_time: state.curr_time,
        }
    }

    pub fn update_main_state(self, state: &mut MainState) {
        state.score = self.score;

        state.rocks.clear();
        state.shots.clear();


        let time_diff = state.curr_time - self.server_time;

        state.curr_time = self.server_time;

        // for now it is safe to assume all the indexes will be correct, 
        // it is impossible to 'delete' players currently.
        while self.players.len() > state.players.len() {
            state.add_player();
        }

        let mut remote_list = self.players;

        for i in (0..remote_list.len()).rev() {
            if state.local_player_index == Some(i) {
                let remote = remote_list.pop().unwrap();
                state.players[i].actor = remote.actor;
                state.players[i].actor.post_deserialize();
                state.players[i].last_shot_at -= time_diff;

            } else {
                state.players[i] = remote_list.pop().unwrap();
                state.players[i].actor.post_deserialize();
                state.players[i].last_shot_at -= time_diff;
            }
        }


        for mut actor in self.actors {
            actor.post_deserialize();
            
            match actor.tag {
                actor::ActorType::Player => {},
                actor::ActorType::Rock => state.rocks.push(actor),
                actor::ActorType::Shot => state.shots.push(actor),
            }
        }
    }
}
