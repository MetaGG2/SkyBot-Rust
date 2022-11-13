use std::time::Duration;

use rand::Rng;
use serenity::utils::Colour;

pub fn rand_color() -> Colour {
    let mut rng = rand::thread_rng();
    Colour::from_rgb(
        rng.gen_range(50..100), 
        rng.gen_range(100..255), 
        rng.gen_range(100..255))   
}

pub fn duration_formatter(duration: Duration) -> String {
    let mut formatted: Vec<String> = Vec::new();
    let mut seconds = duration.as_secs();

    // Hours
    if (seconds / 3600) >= 1 {
        formatted.push(
            format!("{:#?} hours", (seconds / 3600))
        );
        seconds %= 3600;
    }

    // Minutes
    if (seconds / 60) >=1 {
        formatted.push(
            format!("{:#?} minutes", (seconds / 60))
        );
        seconds %= 60;
    }
    
    // Seconds
    if seconds >= 1 {
        formatted.push(
            format!("{:#?} seconds", seconds)
        );
    }

    formatted.join(", ")
}