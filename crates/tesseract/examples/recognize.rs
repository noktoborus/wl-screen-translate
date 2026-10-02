//! Prints the lines recognised in an image: `recognize <models> <image> <language>`.

use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: recognize <models> <image> <language>";
    let (Some(models), Some(image), Some(language)) = (args.next(), args.next(), args.next())
    else {
        return Err(usage.into());
    };
    let image = image::open(image)?.to_rgba8();
    let started = Instant::now();
    let mut engine = tesseract::Engine::load(models.as_ref())?;
    eprintln!(
        "Tesseract {} loaded in {:?}",
        engine.version(),
        started.elapsed()
    );
    let mut lines = Vec::new();
    // The first run reads the model; the second shows the usual time.
    for _ in 0..2 {
        let started = Instant::now();
        lines = engine.recognize(&image, &language)?;
        eprintln!("recognised in {:?}", started.elapsed());
    }
    for line in lines {
        println!(
            "{:>2} {:>3} {:>5.0} {:>5.0} {:>5.0}x{:<3.0} {}",
            line.block, line.paragraph, line.x, line.y, line.width, line.height, line.text
        );
    }
    Ok(())
}
