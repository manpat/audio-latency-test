use std::error::Error;

use sdl2::keyboard::Scancode;
use sdl2::mouse::MouseButton;
use sdl2::pixels::Color;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use std::f32::consts::TAU;

use tracing::{self, instrument};


fn main() -> Result<(), Box<dyn Error>> {
	init_tracy();

	let sdl_ctx = sdl2::init()?;
	let sdl_video = sdl_ctx.video()?;
	let sdl_audio = sdl_ctx.audio()?;

	let window = sdl_video.window("latency-test", 1366, 768)
		.position_centered()
		.resizable()
		.build()?;

	let mut canvas = window.into_canvas().build().unwrap();

	let desired_spec = sdl2::audio::AudioSpecDesired {
		freq: Some(44100),

		channels: Some(1),
		samples: Some(128),
	};

	let trigger = Arc::new(AtomicBool::new(false));
	let mut flash_timer = 0f32;
	let mut frame_timer = std::time::Instant::now();

	let create_submission_worker = |spec: sdl2::audio::AudioSpec| {
		assert!(spec.channels == 1);

		let dt = 1.0 / spec.freq as f32;

		AudioSubmissionWorker {
			trigger: trigger.clone(),
			dt,
			phase: 0.0,
			time: 0.0,
		}
	};

	let audio_device = sdl_audio.open_playback(None, &desired_spec, create_submission_worker)?;
	audio_device.resume();
		
	let mut event_pump = sdl_ctx.event_pump()?;

	'main_loop: loop {
		for event in event_pump.poll_iter() {
			use sdl2::event::Event;

			match event {
				Event::Quit {..} => { break 'main_loop }
				Event::MouseButtonDown { mouse_btn: MouseButton::Left, .. } => {
					trigger.store(true, Ordering::Relaxed);
					flash_timer = 3.0 / 60.0;
					tracing::info!("mouse_trigger");
				}

				Event::KeyDown { scancode: Some(scancode), repeat: false, .. } => {
					match scancode {
						Scancode::Escape => { break 'main_loop }
						_ => {
							trigger.store(true, Ordering::Relaxed);
							flash_timer = 3.0 / 60.0;
							tracing::info!("button_trigger");
						}
					}
				}

				_ => {}
			}
		}

		let color = if flash_timer > 0.0 {
			Color::RGB(255, 0, 255)
		} else {
			Color::RGB(0, 180, 180)
		};

		canvas.set_draw_color(color);
		canvas.clear();

		tracing::info!(tracy.frame_mark=true);
		canvas.present();

		let dt = frame_timer.elapsed();
		frame_timer = std::time::Instant::now();

		flash_timer -= dt.as_secs_f32();
	}

	Ok(())
}



struct AudioSubmissionWorker {
	trigger: Arc<AtomicBool>,
	dt: f32,
	phase: f32,
	time: f32,
}

impl sdl2::audio::AudioCallback for AudioSubmissionWorker {
	type Channel = i16;

	#[instrument(skip_all, name = "AudioSubmissionWorker::callback")]
	fn callback(&mut self, output: &mut [Self::Channel]) {
		tracing::info!("{}", output.len());

		output.fill(0 as _);

		let AudioSubmissionWorker {ref trigger, dt, ref mut phase, ref mut time} = *self;

		if trigger.fetch_and(false, Ordering::Relaxed) {
			tracing::info!("trigger_recv");
			*time = 0.0;
		}

		for sample in output.iter_mut() {
			let osc = (440.0 * TAU * *phase).sin();
			let env = (1.0 - *time * 100.0).max(0.0).powi(2);

			*phase += dt;
			*time += dt;
			*sample = to_sample(osc * env);
		}

		*phase = phase.fract();
	}
}

fn to_sample(v: f32) -> i16 {
	let ub = i16::MAX as f32;
	let lb = i16::MIN as f32;

	(v * ub).clamp(lb, ub) as i16
}



fn init_tracy() {
	use tracing_subscriber::layer::SubscriberExt;

	let subscriber = tracing_subscriber::registry()
		.with(tracing_tracy::TracyLayer::new());

	tracing::subscriber::set_global_default(subscriber)
		.expect("set up the subscriber");
		
	println!("tracy init");
}