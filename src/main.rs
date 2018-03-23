extern crate gl;
extern crate glfw;
extern crate image;
extern crate cgmath;
extern crate rand;

mod util;
mod ico;
mod shader;
mod maze;
mod wall;
mod walker;
mod camera;
mod texture;

use std::cmp;
use std::ffi::CStr;
use std::collections::HashMap;

use cgmath::{Matrix3, Matrix4, Deg, perspective, vec3, InnerSpace};
use glfw::{Action, Context, Key};

use wall::{Wall, WallRenderer, TexType};
use ico::{Ico, IcoRenderer};
use shader::Shader;
use maze::Maze;
use walker::Walker;
use camera::Camera;
use texture::Texture;


const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

type IcoMap = HashMap<(usize, usize), Ico>;

#[derive(Debug)]
enum State {
    Walking,
    Turning,
    Rolling
}

fn main() {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();

    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 3));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));

    if cfg!(target_os = "macos") {
        glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
    }

    let (mut window, events) =
        glfw.create_window(WIDTH,
                           HEIGHT,
                           "Wind95 Maze",
                           glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");

    window.set_key_polling(true);
    window.make_current();

    gl::load_with(|s| window.get_proc_address(s) as *const _);

    // vsync
    //glfw.set_swap_interval(glfw::SwapInterval::None);

    let maze = Maze::new(20, 20);
    maze.print();

    let mut state = State::Walking;

    let (shader_program, textures) = unsafe {
        set_up_shaders()
    };

    let mut wall_renderer = unsafe {
        WallRenderer::new(textures)
    };
    build_walls(&maze, &mut wall_renderer);

    let ico_renderer = unsafe {
        IcoRenderer::new()
    };
    let mut icos = gen_icos(&maze);

    let mut walker = Walker::new(&maze);

    let ratio = WIDTH as f32 / HEIGHT as f32;

    let mut camera = Camera::new(&walker);

    let proj = perspective(Deg(60.0), ratio, 0.1, 100.0);

    let mut frame_count = 0;
    let mut last_second = glfw.get_time();
    let mut last_frame = glfw.get_time();
    let mut next_camera_y = -1.0;

    walker.next();
    while !window.should_close() {
        // input and stuff
        for (_, event) in glfw::flush_messages(&events) {
            handle_window_event(&mut window, event);
        }

        let current_time = glfw.get_time();
        let delta_time = (current_time - last_frame) as f32;
        last_frame = current_time;

        // camera movement
        let completed = match state {
            State::Walking => {
                camera.move_to(walker.to_point(), delta_time)
            }
            State::Turning => {
                let v_dir = walker.direction.to_vec();
                camera.rotate_to(v_dir, delta_time)
            }
            State::Rolling => {
                camera.roll_to(vec3(0.0, next_camera_y, 0.0), delta_time)
            }
        };

        if completed {
            state = match state {
                State::Walking => {
                    walker.next();
                    let v_dir = walker.direction.to_vec();
                    if camera.looking_at(v_dir) {
                        if icos.contains_key(&walker.pos()) {
                            State::Rolling
                        } else {
                            State::Walking
                        }
                    } else {
                        State::Turning
                    }
                }
                State::Turning => {
                    if icos.contains_key(&walker.pos()) {
                        State::Rolling
                    } else {
                        State::Walking
                    }
                }
                State::Rolling => {
                    next_camera_y = if camera.upside_down() {
                        -1.0
                    } else {
                        1.0
                    };
                    icos.remove(&walker.pos());
                    State::Walking
                }
            };
        };

        //handle_input(&window, &mut camera, delta_time * 3.0);

        let view = Matrix4::look_at(camera.pos,
                                    camera.pos + camera.dir,
                                    camera.up);

        // FPS counting
        if (current_time - last_second) > 1.0 {
            last_second = current_time;
            println!("FPS: {}", frame_count);
            frame_count = 0;
        } else {
            frame_count += 1;
        }

        // rendering
        unsafe {
            gl::ClearColor(0.2, 0.3, 0.3, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

            shader_program.set_mat4(c_str!("view"), view);
            shader_program.set_mat4(c_str!("proj"), proj);

            wall_renderer.draw(&shader_program);
            for (_, ico) in &icos {
                ico_renderer.draw(
                    &shader_program, ico, current_time as f32);
            }
        }

        window.swap_buffers();
        glfw.poll_events();
    }
}

fn build_walls(maze: &Maze, wall_renderer: &mut WallRenderer) {

    // north walls
    for j in 0..maze.width {
        let tex = get_rand_tex();
        wall_renderer.add(
            Wall {
                pos: vec3(j as f32 + 0.5, 0.0, 0.0),
                rotate_y: 0.0,
                rotate_x: 0.0,
                texture: tex
            })
    }

    // west walls
    for i in 0..maze.height {
        let tex = get_rand_tex();
        wall_renderer.add(
            Wall {
                pos: vec3(0.0, 0.0, i as f32 + 0.5),
                rotate_y: 90.0,
                rotate_x: 0.0,
                texture: tex
            })
    }

    // inner walls but only east or south
    for i in 0..maze.height {
        for j in 0..maze.width {

            // south wall
            if maze.south(i, j) {
                let tex = get_rand_tex();
                wall_renderer.add(
                    Wall {
                        pos: vec3(j as f32 + 0.5, 0.0, i as f32 + 1.0),
                        rotate_y: 0.0,
                        rotate_x: 0.0,
                        texture: tex
                    })
            }

            // east wall
            if maze.east(i, j) {
                let tex = get_rand_tex();
                wall_renderer.add(
                    Wall {
                        pos: vec3(j as f32 + 1.0, 0.0, i as f32 + 0.5),
                        rotate_y: 90.0,
                        rotate_x: 0.0,
                        texture: tex
                    })
            }

            // ceiling wall
            wall_renderer.add(
                Wall {
                    pos: vec3(j as f32 + 0.5, 0.5, i as f32 + 0.5),
                    rotate_y: 0.0,
                    rotate_x: 90.0,
                    texture: TexType::Ceiling
                });

            // floor wall
            wall_renderer.add(
                Wall {
                    pos: vec3(j as f32 + 0.5, -0.5, i as f32 + 0.5),
                    rotate_y: 0.0,
                    rotate_x: 90.0,
                    texture: TexType::Floor
                });
        }
    }
}

fn gen_icos(maze: &Maze) -> IcoMap {
    // let's say there is 2% of tiles with an icosahedron
    let total = (maze.width - 1) * (maze.height - 1);
    let count = cmp::max(2 * total / 100, 2);
    let indices = rand::seq::sample_indices(
        &mut rand::thread_rng(), total, count);
    let rnd_f = || rand::random::<f32>() * 2.0 - 1.0;

    let mut icos = HashMap::new();

    for x in indices {
        let i = x / (maze.width - 1);
        let j = x % (maze.width - 1);

        icos.insert(
            (i, j),
            Ico {
                pos: vec3(j as f32 + 0.5, 0.0, i as f32 + 0.5),
                axis: vec3(rnd_f(), rnd_f(), rnd_f()).normalize(),
                active: true
            });
    }

    icos
}

fn get_rand_tex() -> TexType {
    if rand::random::<f32>() < 0.9 {
        TexType::Brick
    } else {
        TexType::Thing
    }
}

unsafe fn set_up_shaders() -> (Shader, HashMap<TexType, Texture>) {
    gl::Enable(gl::DEPTH_TEST);
    //gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE);

    // textures
    let mut textures = HashMap::new();
    textures.insert(TexType::Brick, Texture::new("resources/brick.bmp", 0));
    textures.insert(TexType::Thing, Texture::new("resources/thing.bmp", 1));
    textures.insert(TexType::Ceiling, Texture::new("resources/ceiling.bmp", 2));
    textures.insert(TexType::Floor, Texture::new("resources/floor.bmp", 3));

    // shaders
    let shader_program = Shader::new("shaders/vertex.glsl",
                                     "shaders/fragment.glsl");

    shader_program.use_program();
    shader_program.set_vec3(c_str!("color"), vec3(0.8, 0.0, 0.5));

    for (_, texture) in &textures {
        texture.bind();
    }

    (shader_program, textures)
}

fn handle_window_event(window: &mut glfw::Window, event: glfw::WindowEvent) {
    match event {
        glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
            window.set_should_close(true)
        }
        _ => {}
    }
}

fn handle_input(window: &glfw::Window, camera: &mut Camera, speed: f32) {
    let right = camera.dir.cross(camera.up).normalize();
    let turn_speed = 60.0;

    if window.get_key(Key::W) == Action::Press {
        camera.pos += speed * camera.dir;
    }
    if window.get_key(Key::S) == Action::Press {
        camera.pos -= speed * camera.dir;
    }
    if window.get_key(Key::A) == Action::Press {
        camera.pos -= speed * right;
    }
    if window.get_key(Key::D) == Action::Press {
        camera.pos += speed * right;
    }
    if window.get_key(Key::Up) == Action::Press {
        camera.dir = Matrix3::from_axis_angle(right, Deg(speed * turn_speed))
                   * camera.dir;
    }
    if window.get_key(Key::Right) == Action::Press {
        camera.dir = Matrix3::from_angle_y(Deg(speed * -turn_speed))
                   * camera.dir;
    }
    if window.get_key(Key::Down) == Action::Press {
        camera.dir = Matrix3::from_axis_angle(right, Deg(speed * -turn_speed))
                   * camera.dir;
    }
    if window.get_key(Key::Left) == Action::Press {
        camera.dir = Matrix3::from_angle_y(Deg(speed * turn_speed))
                   * camera.dir;
    }
}
