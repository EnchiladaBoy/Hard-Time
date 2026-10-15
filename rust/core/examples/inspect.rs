use hardtime_core::{
    assets::AssetRoot,
    legacy::{Character, Items, NativeOptions, Options, Progress},
    model::Model,
};
use std::{error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Usage: inspect <original or native game folder>")?;
    let root = AssetRoot::open(path)?;
    let bytes = root.read("Data/Options.dat", 1024)?;
    if Options::decode(&bytes)?.encode() != bytes {
        return Err("Options byte roundtrip failed".into());
    }
    if root.resolve("Data/NativeOptions.dat").is_ok() {
        let bytes = root.read("Data/NativeOptions.dat", 1024)?;
        if NativeOptions::decode(&bytes)?.encode() != bytes {
            return Err("Native options byte roundtrip failed".into());
        }
    }
    let mut characters = 0;
    let mut slots = 0;
    for slot in 1..=3 {
        let folder = format!("Data/Slot{slot:02}");
        if root.resolve(format!("{folder}/Progress.dat")).is_err() {
            continue;
        }
        let bytes = root.read(format!("{folder}/Progress.dat"), 4096)?;
        let progress = Progress::decode(&bytes)?;
        if progress.encode() != bytes {
            return Err("Progress byte roundtrip failed".into());
        }
        for id in 1..=progress.status[0] {
            let bytes = root.read(format!("{folder}/Character{id:03}.dat"), 64 * 1024)?;
            if Character::decode(&bytes, progress.status[0] as usize)?.encode() != bytes {
                return Err("Character byte roundtrip failed".into());
            }
            characters += 1;
        }
        let bytes = root.read(format!("{folder}/Items.dat"), 1024 * 1024)?;
        if Items::decode(&bytes)?.encode() != bytes {
            return Err("Items byte roundtrip failed".into());
        }
        slots += 1;
    }
    println!(
        "PASS legacy byte roundtrips: options, {slots} populated slots, {characters} characters"
    );
    let mut models = 0;
    let mut triangles = 0;
    for category in ["World", "Characters", "Weapons"] {
        inspect_models(&root, Path::new(category), &mut models, &mut triangles)?;
    }
    println!(
        "PASS {models} original 3DS files; {triangles} triangles; source assets and saves were read only"
    );
    Ok(())
}

fn inspect_models(
    root: &AssetRoot,
    relative: &Path,
    count: &mut usize,
    triangles: &mut usize,
) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(root.resolve(relative)?)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            continue;
        }
        let relative = relative.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            inspect_models(root, &relative, count, triangles)?;
        } else if entry
            .path()
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("3ds"))
        {
            let model = Model::decode(&root.read(&relative, 64 * 1024 * 1024)?)
                .map_err(|error| format!("{}: {error}", relative.display()))?;
            *count += 1;
            *triangles += model.triangle_count();
            if relative.starts_with("World") {
                let mut min = [f32::MAX; 3];
                let mut max = [f32::MIN; 3];
                for v in model.objects.iter().flat_map(|o| &o.vertices) {
                    for axis in 0..3 {
                        min[axis] = min[axis].min(v[axis]);
                        max[axis] = max[axis].max(v[axis]);
                    }
                }
                println!(
                    "{}: {} objects, {} faces, bounds {min:?} to {max:?}",
                    relative.display(),
                    model.objects.len(),
                    model.triangle_count()
                );
            }
        }
    }
    Ok(())
}
