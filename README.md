# Rust Blaster Multiplayer
A personal project for educational purposes based on [ggez](https://github.com/ggez/ggez) example astroblasto.

The project uses serde for serialization and the net code runs on Rust's TCP socket interface. The game supports an unlimited amount of clients and spectators that can connect at any point in the game.

## Building
This project depends on [rust-sdl2](https://github.com/Rust-SDL2/rust-sdl2). 
The current cargo configuration attempts to build SDL2 locally and link it statically. This requires cmake and a c++ compiler.
If you want to link sdl in a different way see the [rust-sdl2 readme.](https://github.com/Rust-SDL2/rust-sdl2) Good luck.

The rest of the dependencies should be handled by cargo automatically.

Debug builds run ***EXTREMELY*** slow at less than 0.1 FPS. Use release only.

## Running

### Server / Solo:
Simply execute without any parameters. This automatically sets up a server listener.

`cargo run --release`


### Client:
A player can connect using first command-line parameter a character 'c' and then the ip / server to connect to.

`cargo run --release -- c localhost`

### Spectator
For spectator run with 's' as first parameter and then ip / server.

`cargo run --release -- s localhost`

### Multiplayer / Connectivity Notes:
 * You can connect as many clients/spectators as you want at any time. 
 * While connecting and until the player / spectator client fully sync the interface may act in weird ways.
 * You can setup connection parameters through net_setup.json. "transfer_ms" is the network tick time. Make sure all clients use the same net config.
 * To connect over the internet you need to port-forward ports 9942 and 9949.
 * You can change the difficulty of the server by providing a difficulty multiplier as first argument. eg: `cargo run --release 2.5`
 * There is currently no way to cleanly leave the session.
