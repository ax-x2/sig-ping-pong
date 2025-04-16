use libc::{
    c_int, c_void, kill, sigaction, sigemptyset, siginfo_t, ucontext_t, SA_SIGINFO, SIGUSR1,
};
use rand::Rng;
use std::thread::sleep;
use std::time::Duration;

const WIDTH: usize = 76;
const HEIGHT: usize = 20;

// base speed
const BASE_FRAME_DELAY: u64 = 80;
const FRAME_VARIATION: u64 = 40;

// some physics
const MAX_ALLOWED_SPEED: f64 = 10.0;
const MIN_ALLOWED_SPEED: f64 = 5.5;
const PADDLE_CENTER: f64 = (HEIGHT / 2) as f64;
const PADDLE_SIZE: f64 = 3.0;
// rand misses
const MISS_PROBABILITY_BASE: f64 = 0.15;
const DIFFICULTY_SCALING: f64 = 0.08;
const MAX_SCORE: u32 = 11;
const NET_POSITION: usize = WIDTH / 2;

const STATE_SERVE: u8 = 0;
const STATE_RALLY: u8 = 1;
const STATE_POINT_END: u8 = 2;

static mut BALL_X: f64 = 1.0;
static mut BALL_Y: f64 = (HEIGHT / 2) as f64;
static mut BALL_DX: f64 = 1.0;
static mut BALL_DY: f64 = 0.0;
// affects trajectory (flugbahn) and bounce behavior
static mut BALL_SPIN: f64 = 0.0;

static mut PADDLE_LEFT_Y: f64 = (HEIGHT / 2) as f64;
static mut PADDLE_RIGHT_Y: f64 = (HEIGHT / 2) as f64;

static mut SCORE_PING: u32 = 0;
static mut SCORE_PONG: u32 = 0;
static mut RALLY_LENGTH: u32 = 0;
static mut LONGEST_RALLY: u32 = 0;
static mut GAME_STATE: u8 = STATE_SERVE;
// 0 = ping, 1 = pong
static mut SERVING_PLAYER: usize = 0; 
static mut GAME_OVER: bool = false;

unsafe extern "C" fn handle_signal(
    _sig: c_int,
    _info: *mut siginfo_t,
    ucontext: *mut ucontext_t,
) {
    static mut A: u64 = 0;
    let ret = if A % 2 == 0 { flop } else { flip } as *const c_void as i64;
    A += 1;
    (*ucontext).uc_mcontext.gregs[16] = ret;
}

fn draw_board(current_player: &str) {
    print!("\x1B[2J\x1B[H");
    
    let bx;
    let by;
    let left_paddle_y;
    let right_paddle_y;
    let score_ping;
    let score_pong;
    let serving;
    let game_state;
    let rally_length;
    let longest_rally;
    let game_over;
    let ball_dx;
    let ball_dy;
    let ball_speed;
    
    unsafe {
        bx = BALL_X.round() as usize;
        by = BALL_Y.round() as usize;
        left_paddle_y = PADDLE_LEFT_Y.round() as usize;
        right_paddle_y = PADDLE_RIGHT_Y.round() as usize;
        score_ping = SCORE_PING;
        score_pong = SCORE_PONG;
        serving = SERVING_PLAYER;
        game_state = GAME_STATE;
        rally_length = RALLY_LENGTH;
        longest_rally = LONGEST_RALLY;
        game_over = GAME_OVER;
        ball_dx = BALL_DX;
        ball_dy = BALL_DY;
        ball_speed = (BALL_DX * BALL_DX + BALL_DY * BALL_DY).sqrt();
    }
    
    println!("---------------------- alessandrods nerd snippet ----------------------");
    println!("ping: {:<2}  pong: {:<2}  │  current: {:<4}  │ serving: {:<4} │ rally: {:<3}", 
             score_ping, score_pong, current_player, 
             if serving == 0 { "ping" } else { "pong" }, rally_length);
    println!("║ ball speed: {:.2} │ longest rally: {:<3} │ {:<24}", 
             ball_speed, longest_rally, 
             if game_over { "game over!" } else { match game_state {
                 STATE_SERVE => "serving...",
                 STATE_RALLY => "in progress",
                 STATE_POINT_END => "point ended",
                 _ => "",
             }});
    
    let direction = if ball_dx > 0.0 { "→" } else if ball_dx < 0.0 { "←" } else { "-" };
    let vert_dir = if ball_dy > 0.0 { "↓" } else if ball_dy < 0.0 { "↑" } else { "-" };
    println!("ball direction: {}{} │ position: ({},{}) │ ball in {:?} side ", 
             direction, vert_dir, bx, by,
             if bx < NET_POSITION { "pings" } else { "pongs" });
    println!("----------------------------------------------------------------------");
    
    println!("------------------------------------------------------------------------------");
    
    let ping_display_width = WIDTH / 2 - 1;
    let pong_display_width = WIDTH / 2 - 1;
    
    let ball_in_bounds = bx >= 0 && bx < WIDTH && by >= 0 && by < HEIGHT;
    
    for y in 0..HEIGHT {
        print!("║");
        for x in 0..WIDTH {
            if x == 0 && y >= left_paddle_y - 1 && y <= left_paddle_y + 1 {
                print!("▌");
            } else if x == WIDTH - 1 && y >= right_paddle_y - 1 && y <= right_paddle_y + 1 {
                print!("▐");
            } else if x == NET_POSITION {
                print!("│");
            } else if ball_in_bounds && x == bx && y == by {
                print!("●");
            } else {
                print!(" ");
            }
        }
        println!("║");
    }
    
    println!("------------------------------------------------------------------------------");

    if game_over {
        let winner = if score_ping >= MAX_SCORE { "ping" } else { "pong" };
        println!("game is game. winner is {}", winner);
        println!("final score: ping {} - {} pong", score_ping, score_pong);
        println!("longest rally: {} hits", longest_rally);
    }
}

fn visualize_point_end(winner: usize, is_miss: bool, current_player: &str) {
    unsafe {
        draw_board(current_player);
        
        if is_miss {
            if (current_player == "ping" && winner == 1) || (current_player == "pong" && winner == 0) {
                println!("{} missed the ball. point to {}!", 
                        current_player, 
                        if winner == 0 { "ping" } else { "pong" });
            } else {
                println!("point to {} opponent missed the ball.", 
                        if winner == 0 { "ping" } else { "pong" });
            }
        } else if BALL_X < 0.0 {
            println!("ball went out on pings side. point to pong!");
        } else if BALL_X >= WIDTH as f64 {
            println!("ball went out on pongs side. point to ping!");
        } else {
            println!("point to {}!", if winner == 0 { "ping" } else { "pong" });
        }
        
        sleep(Duration::from_millis(1000));
        
        score_point(winner);
        
        draw_board(current_player);
        let score_ping = SCORE_PING;
        let score_pong = SCORE_PONG;
        println!("score: ping {} - {} pong", score_ping, score_pong);
        
        sleep(Duration::from_millis(1000));
        reset_for_serve(SERVING_PLAYER);
    }
}

fn ensure_minimum_ball_speed() {
    unsafe {
        let current_speed = (BALL_DX * BALL_DX + BALL_DY * BALL_DY).sqrt();
        
        if current_speed < MIN_ALLOWED_SPEED {
            let (norm_dx, norm_dy) = if current_speed > 0.0 {
                (BALL_DX / current_speed, BALL_DY / current_speed)
            } else {
   
                let direction = if BALL_X < (WIDTH as f64 / 2.0) { 1.0 } else { -1.0 };
                (direction, rand::rng().random_range(-0.5..0.5))
            };
            
            BALL_DX = norm_dx * MIN_ALLOWED_SPEED;
            BALL_DY = norm_dy * MIN_ALLOWED_SPEED;
            
            if BALL_DX.abs() < 0.1 {
                let direction = if BALL_X < (WIDTH as f64 / 2.0) { 1.0 } else { -1.0 };
                BALL_DX = direction * MIN_ALLOWED_SPEED * 0.8;
                BALL_DY = if BALL_DY == 0.0 { 
                    rand::rng().random_range(-0.3..0.3) 
                } else { 
                    BALL_DY 
                };
            }
        }
    }
}

fn update_paddles() {
    unsafe {
        let target_y = if BALL_DX < 0.0 && BALL_X < WIDTH as f64 / 2.0 {
            let time_to_reach = if BALL_DX != 0.0 { BALL_X / -BALL_DX } else { 0.0 };
            let predicted_y = BALL_Y + BALL_DY * time_to_reach;
            
            predicted_y + rand::rng().random_range(-1.0..1.0)
        } else {
            PADDLE_CENTER + rand::rng().random_range(-1.0..1.0)
        };
        
        let paddle_speed = 0.5;
        if (PADDLE_LEFT_Y - target_y).abs() > 0.1 {
            if PADDLE_LEFT_Y < target_y {
                PADDLE_LEFT_Y += paddle_speed;
            } else {
                PADDLE_LEFT_Y -= paddle_speed;
            }
        }
        
        let target_y = if BALL_DX > 0.0 && BALL_X > WIDTH as f64 / 2.0 {
            let time_to_reach = if BALL_DX != 0.0 { (WIDTH as f64 - BALL_X) / BALL_DX } else { 0.0 };
            let predicted_y = BALL_Y + BALL_DY * time_to_reach;
            
            predicted_y + rand::rng().random_range(-1.0..1.0)
        } else {
            PADDLE_CENTER + rand::rng().random_range(-1.0..1.0)
        };
        
        if (PADDLE_RIGHT_Y - target_y).abs() > 0.1 {
            if PADDLE_RIGHT_Y < target_y {
                PADDLE_RIGHT_Y += paddle_speed;
            } else {
                PADDLE_RIGHT_Y -= paddle_speed;
            }
        }
        
        PADDLE_LEFT_Y = PADDLE_LEFT_Y.max(PADDLE_SIZE).min(HEIGHT as f64 - PADDLE_SIZE);
        PADDLE_RIGHT_Y = PADDLE_RIGHT_Y.max(PADDLE_SIZE).min(HEIGHT as f64 - PADDLE_SIZE);
    }
}

fn random_frame_delay() -> Duration {
    let min = BASE_FRAME_DELAY.saturating_sub(FRAME_VARIATION);
    let max = BASE_FRAME_DELAY + FRAME_VARIATION;
    let delay_ms = rand::rng().random_range(min..=max);
    Duration::from_millis(delay_ms)
}

fn update_ball() {
    unsafe {
        BALL_DY += BALL_SPIN * 0.02;
        
        ensure_minimum_ball_speed();
        
        BALL_X += BALL_DX;
        BALL_Y += BALL_DY;
        
        let started_on_left_side = BALL_X - BALL_DX < NET_POSITION as f64;
        
        if BALL_Y < 1.0 {
            BALL_Y = 1.0;
            BALL_DY = -BALL_DY * 0.95;
            BALL_SPIN *= 0.7;
            
            ensure_minimum_ball_speed();
        }
        
        if BALL_Y > (HEIGHT - 2) as f64 {
            BALL_Y = (HEIGHT - 2) as f64;
            BALL_DY = -BALL_DY * 0.95;
            BALL_SPIN *= 0.7;
            
            ensure_minimum_ball_speed();
        }
        
        static mut CONSECUTIVE_NET_HITS: u8 = 0;
        
        let very_close_to_net = BALL_X.round() as usize == NET_POSITION || 
                                BALL_X.round() as usize == NET_POSITION + 1 ||
                                BALL_X.round() as usize == NET_POSITION - 1;
        
        if very_close_to_net && 
           BALL_Y > 1.0 && BALL_Y < (HEIGHT - 2) as f64 &&
           CONSECUTIVE_NET_HITS < 1 && 
           rand::rng().random_bool(0.15) {
            
            CONSECUTIVE_NET_HITS += 1;
            
            if rand::rng().random_bool(0.2) {
                BALL_DX = -BALL_DX * 0.8;
            } else {
                BALL_DX = BALL_DX * 0.6;
            }
            
            if BALL_DX.abs() < MIN_ALLOWED_SPEED {
                BALL_DX = if BALL_DX < 0.0 { -MIN_ALLOWED_SPEED * 1.2 } else { MIN_ALLOWED_SPEED * 1.2 };
            }
            
            if started_on_left_side {
                BALL_X = (NET_POSITION + 2) as f64;
                if BALL_DX < 0.0 && rand::rng().random_bool(0.7) {
                    BALL_DX = -BALL_DX;
                }
            } else {
                BALL_X = (NET_POSITION - 2) as f64;
                if BALL_DX > 0.0 && rand::rng().random_bool(0.7) {
                    BALL_DX = -BALL_DX;
                }
            }
            
            BALL_DY += rand::rng().random_range(-0.2..0.2);
            
            ensure_minimum_ball_speed();
        } else {
            if !very_close_to_net {
                CONSECUTIVE_NET_HITS = 0;
            }
        }
        
        if started_on_left_side && BALL_X > NET_POSITION as f64 && BALL_DX < 0.0 {
            BALL_DX = -BALL_DX;
        } else if !started_on_left_side && BALL_X < NET_POSITION as f64 && BALL_DX > 0.0 {
            BALL_DX = -BALL_DX;
        }
    }
}

fn handle_paddle_hit(is_left_paddle: bool) {
    unsafe {
        let paddle_y = if is_left_paddle { PADDLE_LEFT_Y } else { PADDLE_RIGHT_Y };
        
        let hit_pos = (BALL_Y - paddle_y) / PADDLE_SIZE;
        
        BALL_DX = -BALL_DX;
        
        let speed = (BALL_DX * BALL_DX + BALL_DY * BALL_DY).sqrt();
        let new_speed = (speed * 1.05).min(MAX_ALLOWED_SPEED);
        
        BALL_DY += hit_pos * 0.8;
        
        BALL_SPIN = hit_pos * 1.5;
        
        let magnitude = (BALL_DX * BALL_DX + BALL_DY * BALL_DY).sqrt();
        if magnitude > 0.0 {
            BALL_DX = (BALL_DX / magnitude) * new_speed;
            BALL_DY = (BALL_DY / magnitude) * new_speed;
        } else {
            BALL_DX = if is_left_paddle { 1.0 } else { -1.0 } * new_speed;
            BALL_DY = rand::rng().random_range(-0.3..0.3);
        }
        
        BALL_DY += rand::rng().random_range(-0.1..0.1);
        
        if is_left_paddle {
            BALL_X = 3.0;
        } else {
            BALL_X = (WIDTH - 4) as f64;
        }
        
        ensure_minimum_ball_speed();
        
        RALLY_LENGTH += 1;
    }
}

fn calculate_miss_probability(paddle_y: f64) -> f64 {
    unsafe {
        let speed = (BALL_DX * BALL_DX + BALL_DY * BALL_DY).sqrt();
        let distance_from_paddle = (BALL_Y - paddle_y).abs();
        
        let mut miss_prob = MISS_PROBABILITY_BASE;
        
        miss_prob += (speed - 1.0) * DIFFICULTY_SCALING;
        
        if distance_from_paddle > PADDLE_SIZE * 0.5 {
            miss_prob += (distance_from_paddle - PADDLE_SIZE * 0.5) * 0.15;
        }
        
        miss_prob.min(0.95).max(0.05)
    }
}

fn handle_potential_miss(is_left_paddle: bool) -> bool {
    unsafe {
        let paddle_y = if is_left_paddle { PADDLE_LEFT_Y } else { PADDLE_RIGHT_Y };
        
        let miss_prob = calculate_miss_probability(paddle_y);
        
        if rand::rng().random_bool(miss_prob) {

            if is_left_paddle {
                BALL_X = -1.0; 
            } else {
                BALL_X = WIDTH as f64 + 1.0; 
            }
            
            let miss_offset = rand::rng().random_range(1.5..2.5);
            if BALL_Y < paddle_y {
                BALL_Y = (paddle_y - miss_offset).max(1.0);
            } else {
                BALL_Y = (paddle_y + miss_offset).min((HEIGHT - 2) as f64);
            }
            
            return true;
        }
        
        false
    }
}

fn reset_for_serve(server: usize) {
    unsafe {
        GAME_STATE = STATE_SERVE;
        SERVING_PLAYER = server;
        RALLY_LENGTH = 0;
        
        if server == 0 {
            BALL_X = 1.0;
            BALL_Y = PADDLE_LEFT_Y;
        } else {
            BALL_X = (WIDTH - 2) as f64;
            BALL_Y = PADDLE_RIGHT_Y;
        }
        BALL_DX = 0.0;
        BALL_DY = 0.0;
        BALL_SPIN = 0.0;
        
        if SCORE_PING >= MAX_SCORE || SCORE_PONG >= MAX_SCORE {
            GAME_OVER = true;
        }
    }
}

fn score_point(winner: usize) {
    unsafe {
        if winner == 0 {
            SCORE_PING += 1;
        } else {
            SCORE_PONG += 1;
        }
        
        if RALLY_LENGTH > LONGEST_RALLY {
            LONGEST_RALLY = RALLY_LENGTH;
        }
        
        GAME_STATE = STATE_POINT_END;
        
        let total_points = SCORE_PING + SCORE_PONG;
        if total_points % 2 == 0 {
            SERVING_PLAYER = 1 - SERVING_PLAYER;
        }
    }
}

fn flip() {
    unsafe {
        if GAME_OVER {
            draw_board("ping");
            sleep(Duration::from_millis(1000));
            kill(0, SIGUSR1);
            loop {
                sleep(Duration::from_millis(BASE_FRAME_DELAY));
            }
        }
        
        if GAME_STATE == STATE_SERVE && SERVING_PLAYER == 0 {
            BALL_X = 1.0;
            BALL_Y = PADDLE_LEFT_Y;
            BALL_DX = rand::rng().random_range(1.0..1.8);
            BALL_DY = rand::rng().random_range(-0.7..0.7);
            GAME_STATE = STATE_RALLY;
            
            ensure_minimum_ball_speed();
        }
        
        let mut point_ended = false;
        let mut winner = 0;
        let mut is_miss = false;
        let mut consecutive_static_frames = 0;
        let mut last_ball_x = BALL_X;
        let mut last_ball_y = BALL_Y;
        
        while !point_ended {
            update_paddles();
            update_ball();
            
            if BALL_X > NET_POSITION as f64 && BALL_DX < 0.0 && 
               last_ball_x <= NET_POSITION as f64 {
                BALL_DX = -BALL_DX;
            }
            
            if (BALL_X - last_ball_x).abs() < 0.01 && (BALL_Y - last_ball_y).abs() < 0.01 {
                consecutive_static_frames += 1;
                if consecutive_static_frames > 5 {
                    ensure_minimum_ball_speed();
                    BALL_DX *= 1.5;
                    consecutive_static_frames = 0;
                }
            } else {
                consecutive_static_frames = 0;
            }
            
            last_ball_x = BALL_X;
            last_ball_y = BALL_Y;
            
            if BALL_X <= 1.0 && BALL_Y >= (PADDLE_LEFT_Y - PADDLE_SIZE) && BALL_Y <= (PADDLE_LEFT_Y + PADDLE_SIZE) {
                if handle_potential_miss(true) {
                    winner = 1;
                    point_ended = true;
                    is_miss = true;
                } else {
                    handle_paddle_hit(true);
                }
            }
            
            if BALL_X < 0.0 && !point_ended {
                winner = 1;
                point_ended = true;
            }
            
            if BALL_X >= WIDTH as f64 && !point_ended {
                winner = 0;
                point_ended = true;
            }
            
            if !point_ended {
                draw_board("PING");
            }
            
            if point_ended {
                break;
            }
            
            sleep(random_frame_delay());
            
            if BALL_X >= NET_POSITION as f64 && BALL_DX > 0.0 {
                break;
            }
        }
        
        if point_ended {
            visualize_point_end(winner, is_miss, "PING");
        }
        
        kill(0, SIGUSR1);
    }
    
    loop {
        sleep(Duration::from_millis(BASE_FRAME_DELAY));
    }
}

fn flop() {
    unsafe {
        if GAME_OVER {
            draw_board("PONG");
            sleep(Duration::from_millis(1000));
            kill(0, SIGUSR1);
            loop {
                sleep(Duration::from_millis(BASE_FRAME_DELAY));
            }
        }
        
        if GAME_STATE == STATE_SERVE && SERVING_PLAYER == 1 {
            BALL_X = (WIDTH - 2) as f64;
            BALL_Y = PADDLE_RIGHT_Y;
            BALL_DX = -rand::rng().random_range(1.0..1.8);
            BALL_DY = rand::rng().random_range(-0.7..0.7);
            GAME_STATE = STATE_RALLY;
            
            ensure_minimum_ball_speed();
        }
        
        let mut point_ended = false;
        let mut winner = 0;
        let mut is_miss = false;
        let mut consecutive_static_frames = 0;
        let mut last_ball_x = BALL_X;
        let mut last_ball_y = BALL_Y;
        
        while !point_ended {
            update_paddles();
            update_ball();
            
            if BALL_X < NET_POSITION as f64 && BALL_DX > 0.0 && 
               last_ball_x >= NET_POSITION as f64 {
                BALL_DX = -BALL_DX;
            }
            
            if (BALL_X - last_ball_x).abs() < 0.01 && (BALL_Y - last_ball_y).abs() < 0.01 {
                consecutive_static_frames += 1;
                if consecutive_static_frames > 5 {
                    ensure_minimum_ball_speed();
                    BALL_DX *= 1.5;
                    consecutive_static_frames = 0;
                }
            } else {
                consecutive_static_frames = 0;
            }
            
            last_ball_x = BALL_X;
            last_ball_y = BALL_Y;
            
            if BALL_X >= (WIDTH - 2) as f64 && 
               BALL_Y >= (PADDLE_RIGHT_Y - PADDLE_SIZE) && 
               BALL_Y <= (PADDLE_RIGHT_Y + PADDLE_SIZE) {
                if handle_potential_miss(false) {
                    winner = 0;
                    point_ended = true;
                    is_miss = true;
                } else {
                    handle_paddle_hit(false);
                }
            }
            
            if BALL_X < 0.0 && !point_ended {
                winner = 1;
                point_ended = true;
            }
            
            if BALL_X >= WIDTH as f64 && !point_ended {
                winner = 0;
                point_ended = true;
            }
            
            if !point_ended {
                draw_board("PONG");
            }
            
            if point_ended {
                break;
            }
            
            sleep(random_frame_delay());
            
            if BALL_X <= NET_POSITION as f64 && BALL_DX < 0.0 {
                break;
            }
        }
        
        if point_ended {
            visualize_point_end(winner, is_miss, "PONG");
        }
        
        kill(0, SIGUSR1);
    }
    
    loop {
        sleep(Duration::from_millis(BASE_FRAME_DELAY));
    }
}

fn main() {
    unsafe {
        SCORE_PING = 0;
        SCORE_PONG = 0;
        RALLY_LENGTH = 0;
        LONGEST_RALLY = 0;
        GAME_STATE = STATE_SERVE;
        SERVING_PLAYER = 0;
        GAME_OVER = false;
        
        PADDLE_LEFT_Y = PADDLE_CENTER;
        PADDLE_RIGHT_Y = PADDLE_CENTER;
        
        reset_for_serve(0);
    }
    
    let mut act: sigaction = unsafe { std::mem::zeroed() };
    act.sa_sigaction = handle_signal as usize;
    act.sa_flags = SA_SIGINFO;
    unsafe {
        sigemptyset(&mut act.sa_mask);
        sigaction(SIGUSR1, &act, std::ptr::null_mut());
    }
    
    print!("\x1B[2J\x1B[H");
    println!("alessandrods optimized nerd snippet");

    
    sleep(Duration::from_millis(1000));
    println!("\nrdy...");
    sleep(Duration::from_millis(1000));
    println!("set...");
    sleep(Duration::from_millis(1000));
    println!("go!");
    sleep(Duration::from_millis(500));
    
    flip();
}
