use crate::actor;
use crate::game_structs;
use actor::{Actor, Vec2Serial};
use ggez::nalgebra::Vector2;
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

/// The struct that is transfered from the client to the server.
/// 
/// Just sending input state works ok only for very low latency and transfer rates.
/// 
/// 

#[derive(Debug, Serialize, Deserialize)]
pub struct NetClientInput {
    pub input_state: InputState,
    pub final_position: Vec2Serial,
    pub shots_made: Vec<Actor>,
}

impl NetClientInput {

    /// Runs on server with the data "self" sent from the client with id "player_id"
    
    // normally you would want to ensure the data a client sends is valid.
    // For the purposes of this project and due to the game being co-op we suppose we can trust the client to not cheat.
    pub fn update_main_state(mut self, player_id: usize, state: &mut MainState) {
        if self.shots_made.len() > 0 {
            state.play_sounds.play_shot = true;
        }

        for mut shot in self.shots_made {
            shot.post_deserialize();
            state.shots.push(shot);
        }
        state.players[player_id].input = self.input_state;

        
        state.players[player_id].actor.pos = Vector2::new(self.final_position.x, self.final_position.y);
    }
    
    /// Runs on client to prepare the struct for sending.
    pub fn make_from_state(state: &mut MainState) -> NetClientInput {
        let player = state.get_local_player().unwrap_or(&state.players[0]);
    
        let mut shots_made = Vec::with_capacity(state.local_shots_made.len());

        for shot in &state.local_shots_made {
            let mut shot = shot.clone();
            shot.pre_serialize();
            shots_made.push(shot);
        }
        
        let r = NetClientInput {
            input_state: state.local_input.clone(),
            final_position: Vec2Serial::from_vec(&player.actor.pos),
            shots_made: shots_made,
        };

        state.local_shots_made.clear();
        r
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
                let _ = remote_list.pop().unwrap();
                //state.players[i].actor = remote.actor;
                //state.players[i].actor.post_deserialize();
                //state.players[i].last_shot_at -= time_diff;

            } else {
                state.players[i] = remote_list.pop().unwrap();
                state.players[i].actor.post_deserialize();
            }
            state.players[i].last_shot_at -= time_diff;
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
