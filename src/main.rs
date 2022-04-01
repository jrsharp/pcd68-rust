#![deny(clippy::all)]
#![forbid(unsafe_code)]

extern crate r68k_emu;
extern crate r68k_tools;
extern crate bdf;
extern crate ndarray;

use log::error;
use pixels::{Pixels, SurfaceTexture};
use std::rc::Rc;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;
use image::GenericImageView;
use image::Pixel;
use r68k_emu::cpu::TestCore;
use r68k_tools::assembler::Assembler;
use r68k_tools::memory::Memory;
use std::io;
use std::io::BufReader;
use r68k_tools::srecords::write_s68;
use ndarray::Array2;

const WIDTH: u32 = 400;
const HEIGHT: u32 = 300;
const TEXT_COLS: u32 = 80;
const TEXT_ROWS: u32 = 23;
const CHAR_WIDTH: u32 = 5;
const CHAR_HEIGHT: u32 = 13;

/// Representation of the screen state.
struct Screen {
    text_buffer: Array2<char>,
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    {

        macro_rules! log {
            ( $( $t:tt )* ) => {
                web_sys::console::log_1(&format!( $( $t )* ).into());
            }
        }

        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init_with_level(log::Level::Trace).expect("error initializing logger");

        wasm_bindgen_futures::spawn_local(run());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();

        pollster::block_on(run());
    }
}

async fn run() {
    let event_loop = EventLoop::new();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("PCD-68")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .expect("WindowBuilder error")
    };

    let window = Rc::new(window);

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        use winit::platform::web::WindowExtWebSys;

        // Retrieve current width and height dimensions of browser client window
        let get_window_size = || {
            let client_window = web_sys::window().unwrap();
            LogicalSize::new(
                client_window.inner_width().unwrap().as_f64().unwrap(),
                client_window.inner_height().unwrap().as_f64().unwrap(),
            )
        };

        let window = Rc::clone(&window);

        // Initialize winit window with current dimensions of browser client
        window.set_inner_size(get_window_size());

        let client_window = web_sys::window().unwrap();

        // Attach winit canvas to body element
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");

        // Listen for resize event on browser client. Adjust winit window dimensions
        // on event trigger
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_e: web_sys::Event| {
            let size = get_window_size();
            window.set_inner_size(size)
        }) as Box<dyn FnMut(_)>);
        client_window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    let mut input = WinitInputHelper::new();
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, window.as_ref());
        Pixels::new_async(WIDTH, HEIGHT, surface_texture)
            .await
            .expect("Pixels error")
    };
    let mut screen = Screen::new();
    screen.updateTextBuffer("Hello World! Here is the first test of the 5x13 font on the (maybe) PCD-68.
        
Document Title
==============
So, how does a simple, ASCII, Markdown file look on this 400x300 framebuffer?.
With the minimal-but-readable(?) 5x13 font, this only gives us a terminal size of 
80x23, but this should be sufficient for a number of applications.

## Craziness!!

So, yeah, this is some more text describing the project: a virtual retro
computer platform that can be emulated in code that is portable enough to run on
an embedded system (esp32), but also on the web as a WebAssembly module.

The 'real' or 'real-world' implementation of this virtual computer hardware can
be realized in physical form as a custom esp32 circuit with display and real
I/O.

The experience of using this 'real' system can then be simulated on the web,
even rendering the framebuffer contents onto a 3D model (using WebGL/three.js),
while using the same actual emulator code used in the real machine.");

    // Set up the 68k cpu, etc.:
    let r68k_asm = Assembler::new();

    let asm = r#"
    ; let's start off with a comment, and then set PC to $1000
    ORG $1000
    ADD.B   #$3,D0
    ADD.B   D0,D1
    ADD.B   #$3,D0
    ADD.B   D0,D1
    ADD.B   #$3,D0
    ADD.B   D0,D1
    ADD.B   #$3,D0
    ADD.B   D0,D1
    CLR.B   D0
    DIVU.W  $0004,D0
    CLR.B   D0
"#;

    //log!("assembly: {}", asm);
    println!("assembly: {}", asm);
    let mut reader = BufReader::new(asm.as_bytes());
    let (end, mem) = r68k_asm.assemble(&mut reader).unwrap();
    let offset = mem.offset();
    let mut r68k_emu = TestCore::new_mem(offset, mem.data());
    //log!("assembled {:06x} - {:06x} and PC is {:06x}", offset, end, r68k_emu.pc);
    println!("assembled {:06x} - {:06x} and PC is {:06x}", offset, end, r68k_emu.pc);
    let mut stdout = io::stdout();
    write_s68(&mut stdout, vec![&mem], offset).unwrap();
    r68k_emu.execute(128);

    println!("And now the PC is {:06x}", r68k_emu.pc);
    println!("CPU status: {:06x}", r68k_emu.status_register());

    event_loop.run(move |event, _, control_flow| {
        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            screen.draw(pixels.get_frame());
            if pixels
                .render()
                .map_err(|e| error!("pixels.render() failed: {}", e))
                .is_err()
            {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                pixels.resize_surface(size.width, size.height);
            }

            // Update internal state and request a redraw
            screen.update();
            window.request_redraw();
        }
    });
}

impl Screen {
    /// Create a new `Screen` instance.
    fn new() -> Self {
        Self {
            text_buffer: Array2::from_elem((TEXT_ROWS as usize, TEXT_COLS as usize), ' ')
        }
    }

    /// Update the text buffer contents
    fn updateTextBuffer(&mut self, contents: &str) {
        let mut row: usize = 0;
        let mut col: usize = 0;
        for (i, c) in contents.chars().enumerate() {
            if (c == '\n') {    // Wrap on newline
                col = 0;
                row += 1;
            } else {
                if (col >= TEXT_COLS as usize) { col = 0; row += 1; }    // line-wrap
                print!("{} -> [{}, {}]", c, row, col);
                self.text_buffer[[row, col]] = c;
                col += 1;
            }
        }
        /*
        for (i, c) in contents.chars().enumerate() {
            let row = i / TEXT_COLS as usize;
            let col = i % TEXT_COLS as usize;
            print!("Char [{}] writing to row [{}] , col [{}]", c, row, col);
            self.text_buffer[[row, col]] = c;
        }
        */

    }

    /// Update the `Screen` internal state;
    fn update(&mut self) {
    }

    /// Draw the `Screen` state to the frame buffer.
    ///
    /// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
    fn draw(&self, frame: &mut [u8]) {

        let font_bytes = include_bytes!("5x13.bdf");
        //let mut reader = BufReader::new(font_bytes as &[u8]);
        //let font = bdf::read(reader);
        let font = bdf::read(font_bytes as &[u8]).expect("Failed to load BDF");

        //log!("bytes {:?}", bytes);

        let bytes = include_bytes!("ren_and_stimpy.png");
        let img = image::load_from_memory(bytes).unwrap();

        //log!("dimensions {:?}", img.dimensions());
        //println!("dimensions {:?}", img.dimensions());

        //log!("font {:?}", font);
        //println!("font {:?}", font);
        
        let black = [0x00, 0x00, 0x00, 0xff];
        let white = [0xff, 0xff, 0xff, 0xff];

        // Draw text buffer:
        let mut row = 0;
        for rowArr in self.text_buffer.genrows() {
            let mut col = 0;
            for c in rowArr {
                let glyph = font.glyphs().get(&c).unwrap_or_else(|| font.glyphs().get(&' ').unwrap());
                for ((x, y), pixel) in glyph.pixels() {
                    // Use skip() to calculate pixel location in 400x300x32 framebuffer
                    let mut frame_iter = frame.chunks_exact_mut(4).skip((row * (WIDTH as usize * CHAR_HEIGHT as usize)) + ((col * CHAR_WIDTH as usize) + x as usize) + (y as usize * WIDTH as usize));
                    //frame_iter.next().expect("Could not find location in frame.").as_deref().unwrap().copy_from_slice(&black);
                    let screen_pixel = frame_iter.next().expect("Could not find location in frame.");
                    if (pixel) {
                        screen_pixel.copy_from_slice(&black);
                    } else {
                        screen_pixel.copy_from_slice(&white);
                    }
                    //println!("pixel: {:?}", pixel);
                }
                col += 1;
                //print!("{}\n", c);
            }
            row += 1;
            //println!();
        }

        /*
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % WIDTH as usize) as u32;
            let y = (i / WIDTH as usize) as u32;
        }
        */

        // Paint R & S:
        /*
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % WIDTH as usize) as u32;
            let y = (i / WIDTH as usize) as u32;

            let (width, height) = img.dimensions();
            let px = img.get_pixel(x, y);
            let rgba = px.channels();
            pixel.copy_from_slice(&rgba);
        }
        */

        /*
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % WIDTH as usize) as i16;
            let y = (i / WIDTH as usize) as i16;

            /*
            let inside_the_box = x >= self.box_x
                && x < self.box_x + BOX_SIZE
                && y >= self.box_y
                && y < self.box_y + BOX_SIZE;
            */

            let inside_the_box = (x + y) % 10 <= 4;

            let rgba = if inside_the_box {
                [0x5e, 0x48, 0xe8, 0xff]
            } else {
                [0x48, 0xb2, 0xe8, 0xff]
            };

            pixel.copy_from_slice(&rgba);
        }
        */
    }
}

