# ascii-rs

This is a project that I started on a whim to convert images into their ascii representration. 

After 2 days of clueless rust programming, it works. Refactor needed. 

My plan is to push this to [crates.io](https://crates.io/), but it still needs more polishing.

## Usage

1. Clone repo

```bash
git clone https://github.com/dddictionary/ascii-rs.git
```

2. Build and run with `cargo`

```bash
cargo run <input image location> <output file location.txt>
```

## TODO's
- I plan on improving the CLI more to support more features. 
- I also plan on including video support
    - Live video to ascii as well as video file formats into ascii
- I plan on supporting HTML embedding to render stuff on the web. 
- Fix the aspect ratio issue when converting. The width of the image is not conserved. 

Insipiration:  
[The Coding Train's Ascii Art in p5js](https://www.youtube.com/watch?v=55iwMYv8tGI)
