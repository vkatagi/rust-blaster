//! Based on ggez's asteroid blaster example
//! Modified for a more refined gameplay experience
extern crate ggez;

extern crate rand;

use ggez::audio;
use ggez::conf;
use ggez::event::{self, EventHandler, Keycode, Mod};
use ggez::graphics;
use ggez::graphics::{Point2, Vector2};
use ggez::nalgebra as na;
use ggez::timer;
use ggez::{Context, ContextBuilder, GameResult};

use std::env;
use std::path;

use serde::{Deserialize, Serialize};

use std::thread;
use std::sync::{Mutex, Arc};

/// *********************************************************************
/// Basic stuff, make some helpers for vector functions.
/// ggez includes the nalgebra math library to provide lots of
/// math stuff  We just add some helpers.
/// **********************************************************************

/// Create a unit vector representing the
/// given angle (in radians)
fn vec_from_angle(angle: f32) -> Vector2 {
    let vx = angle.sin();
    let vy = angle.cos();
    Vector2::new(vx, vy)
}

/// *********************************************************************
/// Now we define our Actor's.
/// An Actor is anything in the game world.
/// We're not *quite* making a real entity-component system but it's
/// pretty close.  For a more complicated game you would want a
/// real ECS, but for this it's enough to say that all our game objects
/// contain pretty much the same data.
/// **********************************************************************
#[derive(Debug, Clone, Serialize, Deserialize)]
enum ActorType {
    Player,
    Rock,
    Shot,
}

// Serialization for our non serializable types.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Vec2Serial {
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
struct ActorSerialIntermediate {
    pub pos: Vec2Serial,
    pub vel: Vec2Serial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Actor {
    tag: ActorType,
    
    #[serde(skip, default = "na::zero")]
    pos: Vector2,
    facing: f32,

    #[serde(skip, default = "na::zero")]
    velocity: Vector2,
    ang_vel: f32,
    bbox_size: f32,

    // I am going to lazily overload "life" with a
    // double meaning:
    // for shots, it is the time left to live,
    // for players and rocks, it is the actual hit points.
    life: f32,

    serial_interm: ActorSerialIntermediate,
}

impl Actor {
    fn pre_serialize(&mut self) {
        self.serial_interm.pos = Vec2Serial::from_floats(self.pos.x, self.pos.y);
        self.serial_interm.vel = Vec2Serial::from_floats(self.velocity.x, self.velocity.y);
    }

    fn post_deserialize(&mut self) {
        self.pos = Vector2::new(self.serial_interm.pos.x, self.serial_interm.pos.y);
        self.velocity = Vector2::new(self.serial_interm.vel.x, self.serial_interm.vel.y);  
    }
}

#[derive(Debug)]
struct Player {
    actor: Actor,
    input: InputState,
    last_shot_at: f32
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

fn create_player_actor() -> Actor {
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

fn create_player() -> Player {
    create_player_from_actor(create_player_actor())
}

fn create_player_from_actor(actor: Actor) -> Player {
    Player {
        actor: actor,
        input: InputState::default(),
        last_shot_at: 0.0,
    }
}


fn create_rock() -> Actor {
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

fn create_shot() -> Actor {
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
/// *********************************************************************
/// Now we make functions to handle physics.  We do simple Newtonian
/// physics (so we do have inertia), and cap the max speed so that we
/// don't have to worry too much about small objects clipping through
/// each other.
///
/// Our unit of world space is simply pixels, though we do transform
/// the coordinate system so that +y is up and -y is down.
/// **********************************************************************

const SHOT_SPEED: f32 = 1100.0;
const SHOT_ANG_VEL: f32 = 0.1;

const PLAYER_SPEED: f32 = 500.0;

// Seconds between shots
const PLAYER_SHOT_TIME: f32 = 0.2;

fn player_handle_input(actor: &mut Actor, input: &InputState, dt: f32) {
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

    actor.pos += point * dt * PLAYER_SPEED;
}

const MAX_PHYSICS_VEL: f32 = 950.0;

fn update_actor_position(actor: &mut Actor, dt: f32) {
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
fn wrap_actor_position(actor: &mut Actor, sx: f32, sy: f32) {
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

fn is_out_of_bounds(actor: &mut Actor, sx: f32, sy: f32) -> bool {
    let screen_x_bounds = sx / 2.0;
    let screen_y_bounds = sy / 2.0;

    actor.pos.x > screen_x_bounds 
        || actor.pos.x < -screen_x_bounds 
        || actor.pos.y > screen_y_bounds
        || actor.pos.y < -screen_y_bounds
}    

fn handle_timed_life(actor: &mut Actor, dt: f32) {
    actor.life -= dt;
}

/// Translates the world coordinate system, which
/// has Y pointing up and the origin at the center,
/// to the screen coordinate system, which has Y
/// pointing downward and the origin at the top-left,
fn world_to_screen_coords(screen_width: u32, screen_height: u32, point: Point2) -> Point2 {
    let width = screen_width as f32;
    let height = screen_height as f32;
    let x = point.x + width / 2.0;
    let y = height - (point.y + height / 2.0);
    Point2::new(x, y)
}

/// **********************************************************************
/// So that was the real meat of our game.  Now we just need a structure
/// to contain the images, sounds, etc. that we need to hang on to; this
/// is our "asset management system".  All the file names and such are
/// just hard-coded.
/// **********************************************************************

struct Assets {
    player_image: graphics::Image,
    shot_image: graphics::Image,
    rock_image: graphics::Image,
    font: graphics::Font,
    shot_sound: audio::Source,
    hit_sound: audio::Source,
}

impl Assets {
    fn new(ctx: &mut Context) -> GameResult<Assets> {
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

    fn actor_image(&mut self, actor: &Actor) -> &mut graphics::Image {
        match actor.tag {
            ActorType::Player => &mut self.player_image,
            ActorType::Rock => &mut self.rock_image,
            ActorType::Shot => &mut self.shot_image,
        }
    }
}

/// **********************************************************************
/// The `InputState` is exactly what it sounds like, it just keeps track of
/// the user's input state so that we turn keyboard events into something
/// state-based and device-independent.
/// **********************************************************************
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct InputState {
    fire: bool,
    up: bool,
    down: bool,
    right: bool,
    left: bool
}


#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct PlaySounds {
    play_hit: bool,
    play_shot: bool,
}

///
/// Networking struct that the client receives from the server.
///
/// 

#[derive(Debug, Serialize, Deserialize)]
struct NetFromServer {
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
                ActorType::Player => state.players.push(create_player_from_actor(actor)),
                ActorType::Rock => state.rocks.push(actor),
                ActorType::Shot => state.shots.push(actor),
            }
        }
    }
}

/// **********************************************************************
/// Now we're getting into the actual game loop.  The `MainState` is our
/// game's "global" state, it keeps track of everything we need for
/// actually running the game.
///
/// Our game objects are simply a vector for each actor type, and we
/// probably mingle gameplay-state (like score) and hardware-state
/// (like `gui_dirty`) a little more than we should, but for something
/// this small it hardly matters.
/// **********************************************************************

struct MainState {
    local_player_index: i32,
    local_input: InputState,
    players: Vec<Player>,
    shots: Vec<Actor>,
    rocks: Vec<Actor>,
    score: i32,
    assets: Assets,
    screen_width: u32,
    screen_height: u32,
    score_display: graphics::Text,
    level_display: graphics::Text,
    start_time: std::time::Duration,
    curr_time: f32,
    difficulty_mult: f32,
    play_sounds: PlaySounds,
}

impl MainState {
    fn new(ctx: &mut Context) -> MainState {
        ctx.print_resource_stats();
        graphics::set_background_color(ctx, (0, 0, 0, 255).into());

        println!("Game resource path: {:?}", ctx.filesystem);

        print_instructions();

        let assets = Assets::new(ctx).expect("Failed to load assets. Terminating");
        let score_disp = graphics::Text::new(ctx, "score", &assets.font).expect("Failed to make text. Terminating");
        let level_disp = graphics::Text::new(ctx, "level", &assets.font).expect("Failed to make text. Terminating");

        let mut players = Vec::new();
        let rocks = Vec::new();

        players.push(create_player());

        let args: std::vec::Vec<String> = env::args().collect();
        let mut diff_mult = 1.0;
        if args.len() > 1 {
            diff_mult = args[1].parse().unwrap_or(1.0);
        }

        println!("Difficulty Multiplier: {:?}", diff_mult);

        let mut s = MainState {
            local_player_index: 0,
            local_input: InputState::default(),
            players: players,
            shots: Vec::new(),
            rocks: rocks,
            score: 0,
            assets,
            screen_width: ctx.conf.window_mode.width,
            screen_height: ctx.conf.window_mode.height,
            score_display: score_disp,
            level_display: level_disp,
            start_time: ggez::timer::get_time_since_start(ctx),
            curr_time: 0.0,
            difficulty_mult: diff_mult,
            play_sounds: PlaySounds::default(),
        };
        
        s.restart_game(ctx);

        s
    }

    fn is_server(&self) -> bool {
        self.local_player_index == 0
    }

    fn fire_player_shot(shots_ref: &mut Vec<Actor>, player: &Player) {
        let player_actor = &player.actor;
        for i in -1..2 {
            let mut shot = create_shot();
            shot.pos = player_actor.pos;
            shot.facing = player_actor.facing;
            let direction = vec_from_angle(shot.facing);

            shot.velocity.x = SHOT_SPEED * direction.x + (i as f32) * SHOT_SPEED / 3.0;
            shot.velocity.y = SHOT_SPEED * direction.y;
            shots_ref.push(shot);
        }
    }

    fn clear_dead_stuff(&mut self) {
        self.shots.retain(|s| s.life > 0.0);
        self.rocks.retain(|r| r.life > 0.0);
    }

    fn restart_game(&mut self, ctx: &ggez::Context) {
        println!("GAME OVER: Time: {:?} | Score: {:?} | On Difficulty: {:?}", self.curr_time, self.score, self.difficulty_mult);

        self.local_input = InputState::default();
        for p in &mut self.players {
            p.last_shot_at = 0.0;
            p.input = InputState::default();
        }
        self.score = 0;
        self.start_time = ggez::timer::get_time_since_start(ctx);
        for shot in &mut self.shots {
            shot.life = 0.0;
        }
        for rock in &mut self.rocks {
            rock.life = 0.0;
        }
    }

    fn handle_collisions(&mut self, ctx: &ggez::Context) {
        let mut should_restart = false;
        for rock in &mut self.rocks {

            for player_obj in &self.players {
                let player = &player_obj.actor;
                let pdistance = rock.pos - player.pos;
                if pdistance.norm() < (player.bbox_size + rock.bbox_size) {
                    should_restart = true;
                }
            }
            
            for shot in &mut self.shots {
                let distance = shot.pos - rock.pos;
                if distance.norm() < (shot.bbox_size + rock.bbox_size) {
                    shot.life = 0.0;
                    rock.life = 0.0;
                    self.score += 1;
                    self.play_sounds.play_hit = true;
                }
            }
        }
        if should_restart {
            self.restart_game(ctx);
            self.play_sounds.play_hit = true;
        }
    }
    
    fn client_handle_sounds(&mut self, ctx: &ggez::Context) {
        for rock in &mut self.rocks {
            for shot in &mut self.shots {
                let distance = shot.pos - rock.pos;
                if distance.norm() < (shot.bbox_size + rock.bbox_size) {
                    self.play_sounds.play_hit = true;
                    return
                }
            }
        }
    }

    fn spawn_rocks(&mut self, delta: f32) {
        let loops = (delta / 0.004).round() as i32;

        let time_mult = self.curr_time * self.difficulty_mult;

        let spawnpercent =  time_mult / 1600.0 + 0.01;
        let speed_mod = f32::powf(time_mult * 4.0, 0.85) + 100.0;
        let mut max_angle = time_mult / 240.0;

        if max_angle > 0.5 {
            max_angle = 0.5;
        }

        for _ in 0..loops {
            if rand::random::<f32>() < spawnpercent {
                let mut rock = create_rock();

                let mut angle = rand::random::<f32>() * max_angle;
                if rand::random::<bool>() {
                    angle = -angle;
                }
                let x_pos = (rand::random::<f32>() * self.screen_width as f32) - self.screen_width as f32 / 2.0;
                let y_pos = (self.screen_height as f32) / 2.0 - 15.0;

                let speed = rand::random::<f32>() * speed_mod + speed_mod / 2.0;
                
                rock.pos = Vector2::new(x_pos, y_pos);
                rock.velocity = vec_from_angle(std::f32::consts::PI + angle) * (speed);
                
                self.rocks.push(rock);
            }
        }
        
    }

    fn update_ui(&mut self, ctx: &mut Context) {
        let str = if self.is_server() { "Server" } else { "Client" };

        let score_str = format!("Score: {}  {}", self.score, str);
        let score_text = graphics::Text::new(ctx, &score_str, &self.assets.font).unwrap();


        let level_str = format!("Time: {}", get_level_time(ctx, self));
        let level_text = graphics::Text::new(ctx, &level_str, &self.assets.font).unwrap();

        self.score_display = score_text;
        self.level_display = level_text;
    }

    fn play_sounds(&mut self) {
        if self.play_sounds.play_hit && !self.assets.hit_sound.playing() {
            let _ = self.assets.hit_sound.play();
        }
        if self.play_sounds.play_shot && !self.assets.shot_sound.playing() {
            let _ = self.assets.shot_sound.play();
        }
        self.clear_sounds();
    }

    fn clear_sounds(&mut self) {
        self.play_sounds = PlaySounds::default();
    }

    fn real_update_server(&mut self, ctx: &mut Context, seconds: f32) -> GameResult<()> {
        self.players[0].input = self.local_input.clone();
   
        for player_obj in &mut self.players {
            player_handle_input(&mut player_obj.actor, &player_obj.input, seconds);
        }
    
        for player_obj in &mut self.players {
            let input = &player_obj.input;
            if input.fire && player_obj.last_shot_at <= self.curr_time - PLAYER_SHOT_TIME {
                player_obj.last_shot_at = self.curr_time;
                MainState::fire_player_shot(&mut self.shots, player_obj);
                self.play_sounds.play_shot = true;
            }
        }

        // Update the physics for all actors.
        // First the player...
        for player_obj in &mut self.players {
            let player = &mut player_obj.actor;
            update_actor_position(player, seconds);

            wrap_actor_position(
                player,
                self.screen_width as f32,
                self.screen_height as f32,
            );
        }
        
        // Then the shots...
        for act in &mut self.shots {
            update_actor_position(act, seconds);

            if is_out_of_bounds(act, self.screen_width as f32, self.screen_height as f32) {
                act.life = 0.0;
            }
            handle_timed_life(act, seconds);
        }

        // And finally the rocks.
        for act in &mut self.rocks {
            update_actor_position(act, seconds);
            if is_out_of_bounds(act, self.screen_width as f32, self.screen_height as f32) {
                act.life = 0.0;
            }
        }

        self.handle_collisions(ctx);
        self.clear_dead_stuff();
        self.spawn_rocks(seconds);
        self.update_ui(ctx);
        Ok(())
    }

    /// Perform interpolation & "prediction"
    fn real_update_client(&mut self, ctx: &mut Context, seconds: f32) -> GameResult<()> {

        if self.players.len() > self.local_player_index as usize {
            self.players[self.local_player_index as usize].input = self.local_input.clone();
        }
        
   
        for player_obj in &mut self.players {
            player_handle_input(&mut player_obj.actor, &player_obj.input, seconds);
        }
    
        for player_obj in &mut self.players {
            let input = &player_obj.input;
            if input.fire && player_obj.last_shot_at <= self.curr_time - PLAYER_SHOT_TIME {
                player_obj.last_shot_at = self.curr_time;
                self.play_sounds.play_shot = true;
            }
        }

        // Update the physics for all actors.
        // First the player...
        for player_obj in &mut self.players {
            let player = &mut player_obj.actor;
            update_actor_position(player, seconds);

            wrap_actor_position(
                player,
                self.screen_width as f32,
                self.screen_height as f32,
            );
        }
        
        // Then the shots...
        for act in &mut self.shots {
            update_actor_position(act, seconds);
        }

        // And finally the rocks.
        for act in &mut self.rocks {
            update_actor_position(act, seconds);
        }

        self.client_handle_sounds(ctx);
        self.update_ui(ctx);
        Ok(())
    }

    fn s_draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        // Our drawing is quite simple.
        // Just clear the screen...
        graphics::clear(ctx);

        // Loop over all objects drawing them...
        {
            let assets = &mut self.assets;
            let coords = (self.screen_width, self.screen_height);
            
            for p_obj in &self.players {
                draw_actor(assets, ctx, &p_obj.actor, coords)?;
            }
            
            for s in &self.shots {
                draw_actor(assets, ctx, s, coords)?;
            }

            for r in &self.rocks {
                draw_actor(assets, ctx, r, coords)?;
            }
        }

        // And draw the GUI elements in the right places.
        let level_dest = graphics::Point2::new(10.0, 10.0);
        let score_dest = graphics::Point2::new(200.0, 10.0);
        graphics::draw(ctx, &self.level_display, level_dest, 0.0)?;
        graphics::draw(ctx, &self.score_display, score_dest, 0.0)?;


        // Play our sound queue
        self.play_sounds();

        // And yield the timeslice
        // This tells the OS that we're done using the CPU but it should
        // get back to this program as soon as it can.
        // This ideally prevents the game from using 100% CPU all the time
        // even if vsync is off.
        // The actual behavior can be a little platform-specific.
        Ok(())
    }

    // Handle key events.  These just map keyboard events
    // and alter our input state appropriately.
    fn s_key_down_event(&mut self, ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        let input_ref = &mut self.local_input;
        match keycode {
            Keycode::Up => {
                input_ref.up = true;
            }
            Keycode::Down => {
                input_ref.down = true;
            }
            Keycode::Left => {
                input_ref.left = true;
            }
            Keycode::Right => {
                input_ref.right = true;
            }
            Keycode::Space => {
                input_ref.fire = true;
            }
            Keycode::Escape => ctx.quit().unwrap(),
            _ => (), // Do nothing
        }
    }

    fn s_key_up_event(&mut self, _ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        let input_ref = &mut self.local_input;
        match keycode {
            Keycode::Up => {
                input_ref.up = false;
            }
            Keycode::Down => {
                input_ref.down = false;
            }
            Keycode::Left => {
                input_ref.left = false;
            }
            Keycode::Right => {
                input_ref.right = false;
            }
            Keycode::Space => {
                input_ref.fire = false;
            }
            _ => (), // Do nothing
        }
    }

}
/// Utility wrapper for level time.
fn get_level_time(ctx: &mut Context, state: &MainState) -> f32 {
    let current = ggez::timer::get_time_since_start(ctx);
    let duration = current - state.start_time;
    duration.as_millis() as f32 / 1000.0
}


/// **********************************************************************
/// A couple of utility functions.
/// **********************************************************************

fn print_instructions() {
    println!();
    println!("Welcome to Rust-Blaster");
    println!();
}

fn draw_actor(
    assets: &mut Assets,
    ctx: &mut Context,
    actor: &Actor,
    world_coords: (u32, u32),
) -> GameResult<()> {
    let (screen_w, screen_h) = world_coords;
    let pos = world_to_screen_coords(screen_w, screen_h, Point2::new(actor.pos.x, actor.pos.y));
    let image = assets.actor_image(actor);
    let drawparams = graphics::DrawParam {
        dest: pos,
        rotation: actor.facing as f32,
        offset: graphics::Point2::new(0.5, 0.5),
        ..Default::default()
    };
    graphics::draw_ex(ctx, image, drawparams)
}


struct StatePtr {
    state: Arc<Mutex<MainState>>
}

impl StatePtr {
    fn new(ctx: &mut Context) -> StatePtr {
        StatePtr {
            state: Arc::new(Mutex::new(MainState::new(ctx))),
        }
    }

    fn get_ref(&mut self) -> StatePtr {
        StatePtr {
            state: self.state.clone()
        }
    }
}

impl EventHandler for StatePtr {
    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        let r = self.state.lock().unwrap().s_draw(ctx);
        graphics::present(ctx);

        thread::sleep(Duration::from_micros(500));
        r
    }

    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {

        const DESIRED_FPS: u32 = 144;
        
        while timer::check_update_time(ctx, DESIRED_FPS) {
            let seconds = 1.0 / (DESIRED_FPS as f32);

            let mut locked_state = self.state.lock().unwrap();          

            if locked_state.is_server() {
                locked_state.curr_time = get_level_time(ctx, &locked_state);
                locked_state.real_update_server(ctx, seconds)?;
            }
            else {
                locked_state.curr_time += seconds;
                locked_state.real_update_client(ctx, seconds)?;
            }
        }

        Ok(())
    }

    fn key_down_event(&mut self, ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        self.state.lock().unwrap().s_key_down_event(ctx, keycode, _keymod, _repeat)
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        self.state.lock().unwrap().s_key_up_event(_ctx, keycode, _keymod, _repeat)
    }
}

/// **********************************************************************
/// Finally our main function!  Which merely sets up a config and calls
/// `ggez::event::run()` with our `EventHandler` type.
/// **********************************************************************

pub fn main() {
    let mut cb = ContextBuilder::new("rust-blaster", "katagis")
        .window_setup(conf::WindowSetup::default().title("Rust Blaster!"))
        .window_mode(conf::WindowMode::default().dimensions(1080, 1080));

    cb = cb.add_resource_path(path::PathBuf::from("resources"));

    let ctx = &mut cb.build().unwrap();
    
    let mut game_ptr = StatePtr::new(ctx);

    let mut net_ptr = game_ptr.get_ref();
    thread::spawn(move || {
        network_main(&mut net_ptr);
    });

    let result = event::run(ctx, &mut game_ptr);

    if let Err(e) = result {
        println!("Error encountered running game: {}", e);
    } else {
        println!("Game exited cleanly.");
    }
}

///
/// Networking Thread
/// 

fn network_main(stateptr: &mut StatePtr) { 
    let mut is_server = false;

    let mut args: std::vec::Vec<String> = env::args().collect();
    if args.len() <= 2 {
        is_server = true;
    }
    let is_server = is_server;

    if !is_server {
        client_main(stateptr, &mut args[2]).expect("Client thread paniced.");
    } else {
        server_main(stateptr).expect("Server thread paniced.");
    }

    
}



use std::net::{TcpListener, TcpStream};
use std::io::prelude::*;
use std::io::BufReader;
use serde::de::DeserializeOwned;
use std::time::Duration;

const TRANSFER_RATE: Duration = Duration::from_millis(16);
const TIMEOUT: Option<Duration> = Some(Duration::from_millis(50));
const PACKET_TTL: u32 = 1;
const NONBLOCKING: bool = false;
const EOP: u8 = 28;
const NODELAY: bool = true;

#[allow(unused_must_use)]
fn configure_stream(stream :&mut TcpStream) {
    stream.set_nodelay(NODELAY);
    stream.set_read_timeout(TIMEOUT);
    stream.set_write_timeout(TIMEOUT);
    stream.set_ttl(PACKET_TTL);
    stream.set_nonblocking(NONBLOCKING);
}

/// Attempts to send the struct in the stream.
fn send_struct<T: Serialize>(stream :&mut TcpStream, data: T) {
    let mut json_send = serde_json::to_vec(&data).expect("Failed to serialize.");
    json_send.push(EOP);
    stream.write_all(&json_send[..]).expect("Write for send_struct Failed.");
}

/// Runs the given Function with the Deserialized struct. 
/// Intended to edit a mutable state capture.
fn recv_update<T: DeserializeOwned>(stream: &mut TcpStream, function: impl Fn(T)) {
    let mut read_buf = BufReader::new(stream);
    let mut json_vec = Vec::new();
    match read_buf.read_until(EOP, &mut json_vec) {
        Ok(_) => {
            let input_data: Result<T, _> = serde_json::from_slice(&json_vec[..json_vec.len()-1]);

            match input_data {
                Ok(data) => function(data),
                Err(_) => {
                    recv_update(read_buf.get_mut(), function);
                }
            }
        },
        Err(_) => { }
    }
}

fn client_main(stateptr: &mut StatePtr, server_addres: &mut String) -> std::io::Result<()> {
    
    let mut recv_stream = TcpStream::connect(format!("{}:9942", server_addres))?;
    let mut send_stream = TcpStream::connect(format!("{}:9949", server_addres))?;

    configure_stream(&mut recv_stream);
    configure_stream(&mut send_stream);

    {
        stateptr.state.lock().unwrap().local_player_index = 1;
    }
    println!("Client connecting!");

    let ptr = stateptr.get_ref();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(TRANSFER_RATE);

            recv_update(&mut recv_stream, |data: NetFromServer| {
                let mut state = ptr.state.lock().unwrap();
                data.update_main_state(&mut state);
            });
        }
    });

    let ptr = stateptr.get_ref();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(TRANSFER_RATE);

            let input_data;
            {
                let state = ptr.state.lock().unwrap();
                input_data = state.local_input.clone();
            }

            send_struct(&mut send_stream, input_data);
        }
    });  
    Ok(())
}

fn server_sender(mut stream: TcpStream, stateptr: StatePtr) {
    configure_stream(&mut stream);

    loop {
        std::thread::sleep(TRANSFER_RATE);

        let mut net_struct;
        {
            let state = stateptr.state.lock().unwrap();
            net_struct = NetFromServer::make_from_state(&state);
        }
        send_struct(&mut stream, net_struct);
    }
}

fn server_recver(mut stream: TcpStream, stateptr: StatePtr) -> std::io::Result<()> {
    configure_stream(&mut stream);
    let player_index;
    {
        let mut state = stateptr.state.lock().unwrap();
        state.players.push(create_player());
        player_index = state.players.len() - 1;
    }
    
    loop {
        std::thread::sleep(TRANSFER_RATE);
        
        recv_update(&mut stream, |data: InputState| {
            match stateptr.state.lock() {
                Ok(ref mut state) => {
                    state.players[player_index].input = data;
                },
                Err(_) => {},
            }
        });
    }
}

fn server_main(stateptr: &mut StatePtr) -> std::io::Result<()> {
    let send_lstener = TcpListener::bind("localhost:9942")?;
    let recv_listener = TcpListener::bind("localhost:9949")?;

    println!("Server!");
    println!("Listening for connections.");
    
    let mut ptr = stateptr.get_ref();
    std::thread::spawn(move || {
        for listen_result in send_lstener.incoming() {
            let this_listen_ref = ptr.get_ref();
            let stream = listen_result.expect("Server Sender Thread Failed.");
            println!("Client Connected: {:?}", stream.peer_addr());
            server_sender(stream, this_listen_ref);
        }
    });

    let mut ptr = stateptr.get_ref();
    std::thread::spawn(move || {
        for listen_result in recv_listener.incoming() {
            let this_listen_ref = ptr.get_ref();
            let stream = listen_result.expect("Server Recv Thread Failed.");
            server_recver(stream, this_listen_ref).expect("Server Recv Thread Failed.");
        }
    });  

    Ok(())
}