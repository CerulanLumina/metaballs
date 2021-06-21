use std::path::PathBuf;

use image::{ImageBuffer, Rgba};
use pixels::SurfaceTexture;
use structopt::StructOpt;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit::dpi::LogicalSize;
use std::io::{stdin, BufRead};
use std::sync::mpsc::{Sender, TryRecvError};
use std::str::FromStr;

use lazy_static::lazy_static;
use winit_input_helper::WinitInputHelper;
use winit::event::VirtualKeyCode;

/// The base metaball size for the provided generation function
const BASE_METABALL_SIZE: f64 = 90.0;

/// The minimum metaball count for the provided generation function
const MIN_METABALL_COUNT: u32 = 3;

/// Load help.txt for outputting to command line
const HELP: &'static str = include_str!("help.txt");

/// The pixel color to draw for being inside the shape
const ON_PIXEL: Rgba<u8> = Rgba([255u8, 0, 0, 255]);

/// The background pixel
const OFF_PIXEL: Rgba<u8> = Rgba([0u8, 0, 0, 255]);

/// Print the help information to STDOUT
fn print_help() {
    println!("{}", HELP);
}

// TODO: Add faster algorithm
/// A naive implementation to render metaballs. This is slow, but works.
fn naive_impl(width: u32, height: u32, metaball_data: &MetaballData) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let math_func = |x, y| {
        // sum the metaball values
        let sum = metaball_data.metaballs.iter().fold(0f64, |acc, metaball| {
            let numerator = metaball.size; // the size of the metaball

            // the distance of the metaball
            let denominator = metaball.location.distance(&Point { x, y }).powf(metaball_data.goo);

            acc + numerator / denominator
        });
        // if the sum if greater than the threshold then draw a pixel
        sum > metaball_data.threshold
    };
    // Use the above closure to determine whether each individual pixel should be on or off
    let image = ImageBuffer::from_fn(width, height, |x, y| {
        if math_func(x, y) {
            ON_PIXEL
        } else {
            OFF_PIXEL
        }
    });
    image
}

fn control_stdin(tx: Sender<ControlCommand>) {
    std::thread::spawn(

        move || {
            // set up reading from stdin
            let stdinput = stdin();
            let mut input = stdinput.lock();
            let mut linebuf = String::new();
            loop {
                // read input line
                input.read_line(&mut linebuf).unwrap();
                let line = linebuf.trim();
                let first_char = line.chars().next();
                if first_char.is_none() { continue; }

                match first_char.unwrap() {
                    // Goo
                    'g' => {
                        match f64::from_str(&line[1..]) {
                            Ok(val) => {tx.send(ControlCommand::Goo(val)).unwrap();}
                            Err(_) => {println!("Unable to parse to float \"{}\"", &line[1..])}
                        }
                    },
                    // Threshold
                    't' => {
                        match f64::from_str(&line[1..]) {
                            Ok(val) => {tx.send(ControlCommand::Threshold(val)).unwrap();}
                            Err(_) => {println!("Unable to parse to float \"{}\"", &line[1..])}
                        }
                    },
                    _ => {
                        println!("Unknown command.")
                    }
                }
                linebuf.clear();

            }
        }
    );
}

/// A control command that can be sent from one thread to another
#[derive(Debug)]
enum ControlCommand {
    /// Adjust goo factor
    Goo(f64),

    /// Adjust the threshold factor
    Threshold(f64),
}

lazy_static! {
    /// The relative points to the center point to draw a cross
    static ref CROSS: Vec<RelPoint> = {
        vec![RelPoint{x: 0, y: 0}, RelPoint{x: 1, y: 0}, RelPoint{x: -1, y: 0}, RelPoint{x: 0, y: 1}, RelPoint{x: 0, y: -1}]
    };
}

#[derive(Default)]
struct RenderOpts {
    pub crosses: bool,
}

/// Use the metaball formula to detect which pixels should be highlighted to create a metaball image
fn render_metaballs(screenbuffer: &mut [u8], metaballs: &MetaballData, opts: &RenderOpts) {
    // draw base metaballs
    let mut meta = naive_impl(256, 256, &metaballs);

    // draw center point indicators
    if opts.crosses {
        for ball in &metaballs.metaballs {
            let pos = ball.location;
            for modifier in CROSS.iter() {
                *(meta.get_pixel_mut((pos.x as i64 + modifier.x) as u32, (pos.y as i64 + modifier.y) as u32)) = Rgba([0u8, 0, 255, 255])
            }
        }
    }




    // copy to buffer
    screenbuffer.copy_from_slice(meta.as_raw().as_slice());
}

/// Main
fn main() {
    print_help();

    // Create Window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(LogicalSize::new(256, 256))
        .with_resizable(false)
        .with_title("Metaballs")
        .build(&event_loop).unwrap();
    let mut input = WinitInputHelper::new();

    // Get window's texture and bind renderer to it
    let surface_texture = SurfaceTexture::new(window.inner_size().width, window.inner_size().height, &window);
    let mut pix = pixels::PixelsBuilder::new(256, 256, surface_texture).enable_vsync(true).build().expect("PixelBuffer");

    // Start thread to listen for commands on STDIN
    let (tx, rx) = std::sync::mpsc::channel();
    control_stdin(tx);


    // Generate and render initial metaballs
    let mut render_opts = RenderOpts::default();
    let mut metadata = MetaballData::from_random(1.6, 0.5, 256, 256);
    render_metaballs(pix.get_frame(), &metadata, &render_opts);

    // Start the window event loop
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested, // If a close is requested
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            Event::RedrawRequested(_) => { // Render the pixel buffer on redraw
                pix.render().unwrap();
            }
            _ => (),
        }
        // Check for received commands from STDIN
        match rx.try_recv() {
            Ok(command) => {
                match command
                {
                    ControlCommand::Goo(goo) => {
                        metadata.goo = goo;
                        println!("Set goo to {}", goo);
                    }
                    ControlCommand::Threshold(threshold) => {
                        metadata.threshold = threshold;
                        println!("Set threshold to {}", threshold);
                    }
                }
                // re-render metaballs and request a redraw
                render_metaballs(pix.get_frame(), &metadata, &render_opts);
                window.request_redraw();
            }
            Err(err) => {
                match err {
                    // No command (no error)
                    TryRecvError::Empty => {},

                    // The command thread panicked for some reason
                    TryRecvError::Disconnected => {println!("STDIN hung up!"); std::process::exit(-1); },
                }
            }
        }

        if input.update(&event) {
            // randomizing control
            if input.key_pressed(VirtualKeyCode::Space) {
                println!("randomizing");
                metadata = MetaballData::from_random(metadata.goo, metadata.threshold, metadata.width, metadata.height);
                render_metaballs(pix.get_frame(), &metadata, &render_opts);
            }

            // center indicator control
            if input.key_pressed(VirtualKeyCode::C) {
                println!("crosses toggled");
                render_opts.crosses = !render_opts.crosses;
                render_metaballs(pix.get_frame(), &metadata, &render_opts);
            }
            // if any input happened request a redraw
            window.request_redraw();
        }

    });
}

/// Defines factors/exponents and positions for rendering a set of metaballs
#[derive(Clone, Debug)]
struct MetaballData {
    pub goo: f64,
    pub threshold: f64,
    pub width: u32,
    pub height: u32,
    pub metaballs: Vec<Metaball>,
}

impl MetaballData {
    /// Generate a bunch of metaballs randomly.
    pub fn from_random(goo: f64, threshold: f64, width: u32, height: u32) -> MetaballData {
        let count = random_count_metaballs();
        let mut metaballs = vec![];
        for _ in 0..count {
            let metaball = Metaball {
                size: centered_random(0.5) * BASE_METABALL_SIZE,
                location: Point {
                    x: (width as f64 * centered_random(0.5)) as u32,
                    y: (height as f64 * centered_random(0.5)) as u32,
                },
            };
            metaballs.push(metaball)
        }
        MetaballData {
            goo,
            width,
            height,
            threshold,
            metaballs,
        }
    }
}

/// Calculates the number of metaballs using RNG
fn random_count_metaballs() -> u32 {
    random_exponential_distribution(0.5).floor() as u32 + MIN_METABALL_COUNT
}

/// Generates a random number following an exponential distribution.
/// This would be like the number of coin flips if on heads flip again, if tails halt.
fn random_exponential_distribution(factor: f64) -> f64 {
    let random = rand::random::<f64>();
    f64::ln(1f64 - random) / (-factor)
}

/// Generates a random number that will be within \[inner / 2, inner * 1.5\]
///
/// Example:
/// ```
/// for _ in 0.1000 {
///     let num = centered_random(0.5);
///     assert!(num >= 0.25 && num <= 0.75);
/// }
/// ```
fn centered_random(inner: f64) -> f64 {
    assert!(inner < 1.0 && inner > 0.0, "Inner should be within (0, 1)");
    let random = rand::random::<f64>();
    random * inner + (inner / 2.0)
}

/// Represents a metaball position and size.
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
struct Metaball {
    pub location: Point,
    pub size: f64,
}

/// Represents a point on an image or screen
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
struct Point {
    pub x: u32,
    pub y: u32,
}

/// Like [Point] but signed integers to allow for negatives. Not used directly for rendering
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
struct RelPoint {
    pub x: i64,
    pub y: i64,
}

impl Point {
    /// Distance to another point
    pub fn distance(&self, other: &Point) -> f64 {
        f64::sqrt(((self.x as f64 - other.x as f64).powf(2f64)) + ((self.y as f64 - other.y as f64).powf(2f64)))
    }
}
