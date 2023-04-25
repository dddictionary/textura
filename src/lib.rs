use std::error::Error;
use std::path::Path;
use image::GenericImageView;
use image::io::Reader as ImageReader;
use image::imageops::FilterType;

pub struct Config {
    pub file_path: String,
}

impl Config {
    pub fn build(args: &[String]) -> Result<Config, &'static str> {
        if args.len() < 2 {
            return Err("Not enough arguments provided!\nUsage: ascii-rs [file_path]");
        }
        let file_path = args[1].clone();
        println!("{} opened.",&file_path);
        Ok(Config{
            file_path
        })
    }
}

fn map_range(val: i32, from: (i32, i32), to: (i32, i32)) -> i32 {
    to.0 + (val-from.0) * (to.1 - to.0) / (from.1 - from.0)
}

pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    let density = vec!["Ñ","@","#","W", "$", "9", "8", "7", "6", "5", "4", "3", "2", "1", "0", "?", "!", "a", "b", "c", ";", ":", "+", "=", "-", ",", ".", "_", " ", " ", " ", " "," "," "," "," "," "];
    ImageReader::open(config.file_path).expect("Image not found.").decode()?.resize(128, 128, FilterType::Gaussian).save(Path::new("ferris.png")).unwrap();    
    let gray_image = ImageReader::open("ferris.png").expect("Image not found.").decode()?.grayscale().clone().into_luma8();

    let (width, height) = (gray_image.width(), gray_image.height());
    for column in 0..height {
        for row in 0..width {
        // let mut output = String::new();
            let pixel = gray_image.get_pixel(row, column);
            let luma = pixel[0];
            let index = map_range(luma as i32, (0, 255), (density.len() as i32 - 1, 0));
            print!("{}", density[index as usize]);
        }
        println!();
    }
    Ok(())
}