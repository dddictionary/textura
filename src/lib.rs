use std::error::Error;
use std::fs;
use std::path::Path;
use image::io::Reader as ImageReader;
use image::imageops::FilterType;

pub struct AsciiConverter {
    pub input_path: String,
    pub output_path: String,
}

impl AsciiConverter {
    pub fn build(args: &[String]) -> Result<AsciiConverter, &'static str> {
        if args.len() < 3 {
            return Err("Not enough arguments provided!\nUsage: ascii-rs [image_path] [output_path]");
        }
        let input_path = args[1].clone();
        let output_path = args[2].clone();
        Ok(AsciiConverter{
            input_path,
            output_path,
        })
    }
}

fn map_range(val: i32, from: (i32, i32), to: (i32, i32)) -> i32 {
    to.0 + (val-from.0) * (to.1 - to.0) / (from.1 - from.0)
}

pub fn run(config: AsciiConverter) -> Result<(), Box<dyn Error>> {
    let output_path = Path::new(&config.output_path);
    if output_path.extension().unwrap() != "txt" {
        return Err("Output file must be a .txt file.".into());
    }
    
    let density = vec!["Ñ","@","#","W", "$", "9", "8", "7", "6", "5", "4", "3", "2", "1", "0", "?", "!", "a", "b", "c", ";", ":", "+", "=", "-", ",", ".", "_", " ", " ", " ", " "," "," "," "," "," "];
    let gray_image = ImageReader::open(&config.input_path).expect("Image not found.").decode()?.resize(256, 256, FilterType::Gaussian).grayscale().clone().into_luma8();    
    println!("Opened {}.",&config.input_path);

    let (width, height) = (gray_image.width(), gray_image.height());
    let mut output = String::new();
    for column in 0..height {
        for row in 0..width {
            let pixel = gray_image.get_pixel(row, column);
            let luma = pixel[0];
            let index = map_range(luma as i32, (0, 255), (density.len() as i32 - 1, 0));
            output += &format!("{}", density[index as usize]);
        }
        output += "\n";
    }

    

    fs::write(&config.output_path, output)?;
    println!("Saved ascii drawing to {}.", &config.output_path);

    Ok(())
}
