use ggez::graphics::Vector2;
use ggez::nalgebra as na;
use serde::{Deserialize, Serialize};

use ggez::audio;
use ggez::graphics;

use ggez::{Context, GameResult};

const SHOT_ANG_VEL: f32 = 0.1;
/// *********************************************************************
/// Now we define our Actor's.
/// An Actor is anything in the game world.
/// We're not *quite* making a real entity-component system but it's
/// pretty close.  For a more complicated game you would want a
/// real ECS, but for this it's enough to say that all our game objects
/// contain pretty much the same data.
/// **********************************************************************
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActorType {
    Player,
    Rock,
    Shot,
}

// Serialization for our non serializable types.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Vec2Serial {
    pub x: f32,
    pub y: f32,
}

impl Vec2Serial {
    fn from_floats(x: f32, y: f32) -> Vec2Serial {
        Vec2Serial {
            x,
            y,
        }
    }
}

// Serialization for our non serializable types.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActorSerialIntermediate {
    pub pos: Vec2Serial,
    pub vel: Vec2Serial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    pub tag: ActorType,
    
    #[serde(skip, default = "na::zero")]
    pub pos: Vector2,
    pub facing: f32,

    #[serde(skip, default = "na::zero")]
    pub velocity: Vector2,
    pub ang_vel: f32,
    pub bbox_size: f32,

    // I am going to lazily overload "life" with a
    // double meaning:
    // for shots, it is the time left to live,
    // for players and rocks, it is the actual hit points.
    pub life: f32,

    serial_interm: ActorSerialIntermediate,
}

impl Actor {
    pub fn pre_serialize(&mut self) {
        self.serial_interm.pos = Vec2Serial::from_floats(self.pos.x, self.pos.y);
        self.serial_interm.vel = Vec2Serial::from_floats(self.velocity.x, self.velocity.y);
    }

    pub fn post_deserialize(&mut self) {
        self.pos = Vector2::new(self.serial_interm.pos.x, self.serial_interm.pos.y);
        self.velocity = Vector2::new(self.serial_interm.vel.x, self.serial_interm.vel.y);  
    }
}

const PLAYER_LIFE: f32 = 1.0;
const SHOT_LIFE: f32 = 2.0;
const ROCK_LIFE: f32 = 1.0;

const PLAYER_BBOX: f32 = 12.0;
const ROCK_BBOX: f32 = 12.0;
const SHOT_BBOX: f32 = 6.0;

/// *********************************************************************
/// Now we have some constructor functions for different game objects.
/// **********************************************************************

impl Actor {
    pub fn create_player_actor() -> Actor {
        Actor {
            tag: ActorType::Player,
            pos: na::zero(),
            facing: 0.,
            velocity: na::zero(),
            ang_vel: 0.,
            bbox_size: PLAYER_BBOX,
            life: PLAYER_LIFE,
            serial_interm: ActorSerialIntermediate::default(),
        }
    }

    pub fn create_rock() -> Actor {
        Actor {
            tag: ActorType::Rock,
            pos: na::zero(),
            facing: 0.,
            velocity: na::zero(),
            ang_vel: 0.,
            bbox_size: ROCK_BBOX,
            life: ROCK_LIFE,
            serial_interm: ActorSerialIntermediate::default(),
        }
    }

    pub fn create_shot() -> Actor {
        Actor {
            tag: ActorType::Shot,
            pos: na::zero(),
            facing: 0.,
            velocity: na::zero(),
            ang_vel: SHOT_ANG_VEL,
            bbox_size: SHOT_BBOX,
            life: SHOT_LIFE,
            serial_interm: ActorSerialIntermediate::default(),
        }
    }
}


#[derive(Debug)]
pub struct Player {
    pub actor: Actor,
    pub input: InputState,
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
        }
    }
    
    pub fn tick_input(&mut self, delta: f32) {
        //actor.facing += dt * PLAYER_TURN_RATE * input.xaxis;
        fn bool_to_f(v: bool) -> f32 {
            if v {
                return 1.0;
            }
            return 0.0;
        }

        let point = Vector2::new(
            bool_to_f(input.right) * 1.0
        + bool_to_f(input.left) * -1.0
        , 
            bool_to_f(input.up) * 1.0
        + bool_to_f(input.down) * -1.0
        );

        actor.pos += point * delta * PLAYER_SPEED;
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

///
/// Networking struct that the client receives from the server.
///
/// 

#[derive(Debug, Serialize, Deserialize)]
pub struct NetFromServer {
    actors: Vec<Actor>,
    score: i32,
    time_offset: f32,
    play_sounds: PlaySounds,
}

impl NetFromServer {
    fn make_from_state(state: &MainState) -> NetFromServer {
        let mut actors = Vec::new();

        for player in &state.players {
            actors.push(player.actor.clone());
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
            actors: actors,
            score: state.score,
            time_offset: state.curr_time,
            play_sounds: state.play_sounds.clone()
        }
    }

    fn update_main_state(self, state: &mut MainState) {
        state.score = self.score;

        state.players.clear();
        state.rocks.clear();
        state.shots.clear();

        //state.play_sounds = self.play_sounds;
                    //self.clear_sounds();

        for mut actor in self.actors {
            actor.post_deserialize();
            match actor.tag {
                ActorType::Player => state.players.push(Player::from_actor(actor)),
                ActorType::Rock => state.rocks.push(actor),
                ActorType::Shot => state.shots.push(actor),
            }
        }
    }
}



const MAX_PHYSICS_VEL: f32 = 950.0;

pub fn update_actor_position(actor: &mut Actor, dt: f32) {
    // Clamp the velocity to the max efficiently
    let norm_sq = actor.velocity.norm_squared();
    if norm_sq > MAX_PHYSICS_VEL.powi(2) {
        actor.velocity = actor.velocity / norm_sq.sqrt() * MAX_PHYSICS_VEL;
    }
    let dv = actor.velocity * (dt);
    actor.pos += dv;
    actor.facing += actor.ang_vel;
}

/// Takes an actor and wraps its position to the bounds of the
/// screen, so if it goes off the left side of the screen it
/// will re-enter on the right side and so on.
pub fn wrap_actor_position(actor: &mut Actor, sx: f32, sy: f32) {
    let screen_x_bounds = sx / 2.0;
    let screen_y_bounds = sy / 2.0;
    if actor.pos.x > screen_x_bounds {
        actor.pos.x -= sx;
    } else if actor.pos.x < -screen_x_bounds {
        actor.pos.x += sx;
    };
    if actor.pos.y > screen_y_bounds {
        actor.pos.y -= sy;
    } else if actor.pos.y < -screen_y_bounds {
        actor.pos.y += sy;
    }
}

pub fn is_out_of_bounds(actor: &mut Actor, sx: f32, sy: f32) -> bool {
    let screen_x_bounds = sx / 2.0;
    let screen_y_bounds = sy / 2.0;

    actor.pos.x > screen_x_bounds 
        || actor.pos.x < -screen_x_bounds 
        || actor.pos.y > screen_y_bounds
        || actor.pos.y < -screen_y_bounds
}    

pub fn handle_timed_life(actor: &mut Actor, dt: f32) {
    actor.life -= dt;
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
        match actor.tag {
            ActorType::Player => &mut self.player_image,
            ActorType::Rock => &mut self.rock_image,
            ActorType::Shot => &mut self.shot_image,
        }
    }
}