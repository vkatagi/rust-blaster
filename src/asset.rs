use std::path;
use std::collections::HashMap;
use ggez::Context;


use ggez::graphics::Image as ImageAsset;
use ggez::graphics::Font as FontAsset;
use ggez::audio::Source as SoundAsset;


trait LoadableAsset {
    fn new(ctx: &mut Context, file: &'static str) -> ggez::GameResult<Self> where Self: Sized;
}

impl LoadableAsset for ImageAsset {
    fn new(ctx: &mut Context, file: &'static str) -> ggez::GameResult<ImageAsset> {
        ImageAsset::new(ctx, file)
    }
}

impl LoadableAsset for FontAsset {
    fn new(ctx: &mut Context, file: &'static str) -> ggez::GameResult<FontAsset> {
        FontAsset::new(ctx, file, 18)
    }
}

impl LoadableAsset for SoundAsset {
    fn new(ctx: &mut Context, file: &'static str) -> ggez::GameResult<SoundAsset> {
        SoundAsset::new(ctx, file)
    }
}

#[derive(Debug)]
struct AssetLibrary<T: LoadableAsset> {
    // TODO: IMPR: This probably should be just copmile-time strings. &'static str (?) as keys
    assets: HashMap<String, T>,
}

// Look into why do we have to specify all this info 
impl<T> AssetLibrary<T> where T: LoadableAsset {

    fn new() -> Self {
        Self {
            assets: HashMap::new()
        }
    }

    fn load(&mut self, ctx: &mut Context, file: &'static str) -> ggez::GameResult<()> {
        let asset = LoadableAsset::new(ctx, file)?;
        self.assets.insert(file.to_owned(), asset);
        Ok(())
    }  

    fn get<S>(&self, name: S) -> Option<&T> where S: Into<String> {
        return self.assets.get(&name.into());
    }
}

#[derive(Debug)]
pub struct Assets {
    images: AssetLibrary<ImageAsset>,
    fonts: AssetLibrary<FontAsset>,
    sounds: AssetLibrary<SoundAsset>,
}

impl Assets {
    
    pub fn new(ctx: &mut ggez::Context, files: Vec<&'static str>) -> Self {
        let mut images = AssetLibrary::new();
        let mut fonts = AssetLibrary::new();
        let mut sounds = AssetLibrary::new();
        
        for f in files {
            match &f[f.len()-3..] {
                "png" | "jpg" => images.load(ctx, f).unwrap(),
                "ogg" => sounds.load(ctx, f).unwrap(),
                "ttf" => fonts.load(ctx, f).unwrap(),
                &_ => {}
            }
        }
        
        Self {
            images: images,
            fonts: fonts,
            sounds: sounds,
        }
    }

    // I would prefer get<ImageAsset>("myasset.png");
    // but I could not even find if its possible to do this currently in rust.

    pub fn get_image<S>(&self, name: S) -> Option<&ImageAsset> where S: Into<String> {
        self.images.get(name)
    }

    pub fn get_font<S>(&self, name: S) -> Option<&FontAsset> where S: Into<String> {
        self.fonts.get(name)
    }

    pub fn get_sound<S>(&self, name: S) -> Option<&SoundAsset> where S: Into<String> {
        self.sounds.get(name)
    }
}