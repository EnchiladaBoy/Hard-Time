use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    // Original, code-drawn icon: the installer contains no original game art.
    let mut bmp = Vec::new();
    for n in [40_u32, 48, 96] {
        bmp.extend(n.to_le_bytes());
    }
    for n in [1_u16, 32] {
        bmp.extend(n.to_le_bytes());
    }
    for n in [0_u32, 48 * 48 * 4, 0, 0, 0, 0] {
        bmp.extend(n.to_le_bytes());
    }
    for y in (0..48).rev() {
        for x in 0..48 {
            let orange = (9..39).contains(&y)
                && ((10..16).contains(&x)
                    || (32..38).contains(&x)
                    || ((16..32).contains(&x) && (21..27).contains(&y)));
            bmp.extend(if orange {
                [85, 148, 237, 255]
            } else {
                [35, 27, 20, 255]
            });
        }
    }
    bmp.extend([0_u8; 8 * 48]);
    let mut ico = vec![0, 0, 1, 0, 1, 0, 48, 48, 0, 0, 1, 0, 32, 0];
    ico.extend((bmp.len() as u32).to_le_bytes());
    ico.extend(22_u32.to_le_bytes());
    ico.extend(bmp);
    fs::write(out.join("setup.ico"), ico).unwrap();
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("app.manifest");
    let escaped = |p: PathBuf| p.to_string_lossy().replace('\\', "\\\\");
    let rc = out.join("setup.rc");
    fs::write(
        &rc,
        format!(
            "1 24 \"{}\"\n1 ICON \"{}\"\n",
            escaped(manifest),
            escaped(out.join("setup.ico"))
        ),
    )
    .unwrap();
    embed_resource::compile(rc, embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
