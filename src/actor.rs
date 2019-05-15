use ggez::graphics::Vector2;
use ggez::nalgebra as na;
use serde::{Deserialize, Serialize};

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

    pub kill: bool,
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

const PLAYER_BBOX: f32 = 12.0;
const ROCK_BBOX: f32 = 12.0;
const SHOT_BBOX: f32 = 6.0;

const SHOT_ANG_VEL: f32 = 0.1;
const MAX_PHYSICS_VEL: f32 = 950.0;


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
            kill: false,
            serial_interm: ActorSerialIntermediate::default(),
        }
    }

    pub fn create_rock() -> Actor {
        Actor {
            tag: ActorType::Rock,
            pos: na::zero(),
            facing: 0.0,
            velocity: na::zero(),
            ang_vel: rand::random::<f32>() * 0.02,
            bbox_size: ROCK_BBOX,
            kill: false,
            serial_interm: ActorSerialIntermediate::default(),
        }
    }

    pub fn create_shot() -> Actor {
        Actor {
            tag: ActorType::Shot,
            pos: na::zero(),
            facing: 0.0,
            velocity: na::zero(),
            ang_vel: SHOT_ANG_VEL,
            bbox_size: SHOT_BBOX,
            kill: false,
            serial_interm: ActorSerialIntermediate::default(),
        }
    }

    pub fn tick_physics(&mut self, delta: f32) {
        // Clamp the velocity to the max efficiently
        let norm_sq = self.velocity.norm_squared();
        if norm_sq > MAX_PHYSICS_VEL.powi(2) {
            self.velocity = self.velocity / norm_sq.sqrt() * MAX_PHYSICS_VEL;
        }
        let dv = self.velocity * (delta);
        self.pos += dv;
        self.facing += self.ang_vel;
    }

    /// Takes an actor and wraps its position to the bounds of the
    /// screen, so if it goes off the left side of the screen it
    /// will re-enter on the right side and so on.
    pub fn wrap_position(&mut self, sx: f32, sy: f32) {
        let screen_x_bounds = sx / 2.0;
        let screen_y_bounds = sy / 2.0;
        if self.pos.x > screen_x_bounds {
            self.pos.x -= sx;
        } else if self.pos.x < -screen_x_bounds {
            self.pos.x += sx;
        };
        if self.pos.y > screen_y_bounds {
            self.pos.y -= sy;
        } else if self.pos.y < -screen_y_bounds {
            self.pos.y += sy;
        }
    }

    pub fn is_out_of_bounds(&self, sx: f32, sy: f32) -> bool {
        let screen_x_bounds = sx / 2.0;
        let screen_y_bounds = sy / 2.0;

        self.pos.x > screen_x_bounds 
            || self.pos.x < -screen_x_bounds 
            || self.pos.y > screen_y_bounds
            || self.pos.y < -screen_y_bounds
    }
}