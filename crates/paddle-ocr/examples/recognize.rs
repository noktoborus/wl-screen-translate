//! Prints the lines recognised in an image: `recognize <models> <image> <script>`.

use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: recognize <models> <image> <script>";
    let (Some(models), Some(image), Some(script)) = (args.next(), args.next(), args.next()) else {
        return Err(usage.into());
    };
    let image = image::open(image)?.to_rgba8();
    let started = Instant::now();
    let mut engine = paddle_ocr::Engine::load(models.as_ref())?;
    eprintln!("loaded in {:?}", started.elapsed());
    let mut lines = Vec::new();
    // The first run warms the runtime up; the second shows the usual time.
    for _ in 0..2 {
        let started = Instant::now();
        lines = engine.recognize(&image, &script)?;
        eprintln!("recognised in {:?}", started.elapsed());
    }
    for line in lines {
        println!(
            "{:>3} {:>5.0} {:>5.0} {:>5.0}x{:<3.0} {}",
            line.paragraph, line.x, line.y, line.width, line.height, line.text
        );
    }
    Ok(())
}
