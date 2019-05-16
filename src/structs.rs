use ggez::graphics::Vector2;
use serde::{Deserialize, Serialize};

use ggez::audio;
use ggez::graphics;

use ggez::{Context, GameResult};

use crate::actor;
use actor::Actor;

use std::sync::{Mutex, Arc};


const PLAYER_SPEED: f32 = 500.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub actor: Actor,
    pub input: InputState,
    pub index: u32,

    #[serde(skip)]
    pub last_shot_at: f32
}

impl Player {
    pub fn create() -> Player {
        Player::from_actor(Actor::create_player_actor())
    }

    pub fn from_actor(actor: Actor) -> Player {
        Player {
            actor: actor,
            input: InputState::default(),
            last_shot_at: 0.0,
            index: 0
        }
    }
    
    pub fn tick_input(&mut self, delta: f32) {
        //actor.facing += dt * PLAYER_TURN_RATE * input.xaxis;
        fn bool_to_f(v: bool) -> f32 {
            if v { 1.0 } else { 0.0 }
        }

        let point = Vector2::new(
        bool_to_f(self.input.right) * 1.0
        + bool_to_f(self.input.left) * -1.0
        , 
        bool_to_f(self.input.up) * 1.0
        + bool_to_f(self.input.down) * -1.0
        );

        self.actor.pos += point * delta * PLAYER_SPEED;
    }

}



/// **********************************************************************
/// The `InputState` is exactly what it sounds like, it just keeps track of
/// the user's input state so that we turn keyboard events into something
/// state-based and device-independent.
/// **********************************************************************
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InputState {
    pub fire: bool,
    pub up: bool,
    pub down: bool,
    pub right: bool,
    pub left: bool
}

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


///
/// Networking struct that the client receives from the server.
///
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
            if state.local_player_index == i {
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

// TODO: refactor
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PlaySounds {
    pub play_hit: bool,
    pub play_shot: bool,
}

/// Assets

pub struct Assets {
    pub player_image: graphics::Image,
    pub shot_image: graphics::Image,
    pub rock_image: graphics::Image,
    pub font: graphics::Font,
    pub shot_sound: audio::Source,
    pub hit_sound: audio::Source,
}

impl Assets {
    pub fn new(ctx: &mut Context) -> GameResult<Assets> {
        let player_image = graphics::Image::new(ctx, "/player.png")?;
        let shot_image = graphics::Image::new(ctx, "/shot.png")?;
        let rock_image = graphics::Image::new(ctx, "/rock.png")?;
        let font = graphics::Font::new(ctx, "/DejaVuSerif.ttf", 18)?;

        let shot_sound = audio::Source::new(ctx, "/pew.ogg")?;
        let hit_sound = audio::Source::new(ctx, "/boom.ogg")?;
        Ok(Assets {
            player_image,
            shot_image,
            rock_image,
            font,
            shot_sound,
            hit_sound,
        })
    }

    pub fn actor_image(&mut self, actor: &Actor) -> &mut graphics::Image {
        use actor::ActorType;
        match actor.tag {
            ActorType::Player => &mut self.player_image,
            ActorType::Rock => &mut self.rock_image,
            ActorType::Shot => &mut self.shot_image,
        }
    }
}

pub struct MainState {
    pub local_player_index: usize,
    pub local_input: InputState,
    pub players: Vec<Player>,
    pub shots: Vec<Actor>,
    pub rocks: Vec<Actor>,
    pub score: i32,
    pub assets: Assets,
    pub screen_width: u32,
    pub screen_height: u32,
    pub score_display: graphics::Text,
    pub level_display: graphics::Text,
    pub start_time: std::time::Instant,
    pub curr_time: f32,
    pub difficulty_mult: f32,
    pub play_sounds: PlaySounds,
}

pub struct StatePtr {
    pub state: Arc<Mutex<MainState>>
}

impl StatePtr {
    pub fn new(ctx: &mut Context) -> StatePtr {
        StatePtr {
            state: Arc::new(Mutex::new(MainState::new(ctx))),
        }
    }

    pub fn get_ref(&mut self) -> StatePtr {
        StatePtr {
            state: self.state.clone()
        }
    }
}